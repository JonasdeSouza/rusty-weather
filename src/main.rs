use axum::{
    extract::State,
    response::Html,
    routing::get,
    Router,
};
use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use serde::Deserialize;
use std::{sync::{Arc, Mutex}, time::Duration};

// ATUALIZAÇÃO: Adicionamos umidade e pressão.
// O Serde vai procurar chaves no JSON com esses mesmos nomes.
#[derive(Deserialize, Debug, Clone)]
struct SensorData {
    temperatura: f64,
    umidade: f64,
    pressao: f64,
}

// Implementamos um valor padrão para quando o programa iniciar e não tiver dados ainda.
impl Default for SensorData {
    fn default() -> Self {
        Self {
            temperatura: 0.0,
            umidade: 0.0,
            pressao: 0.0, // Pressão ao nível do mar padrão é ~1013 hPa
        }
    }
}

type SharedState = Arc<Mutex<SensorData>>;

#[tokio::main]
async fn main() {
    // 1. Inicializar o Estado Compartilhado
    let estado_compartilhado = Arc::new(Mutex::new(SensorData::default()));

    // 2. Configuração do MQTT
    let mut mqttoptions = MqttOptions::new("rust-backend-v2", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    client
        .subscribe("sensores/esp32", QoS::AtLeastOnce)
        .await
        .unwrap();

    // 3. Loop MQTT (Processamento em Background)
    let estado_para_mqtt = estado_compartilhado.clone();
    
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let Event::Incoming(Packet::Publish(p)) = notification {
                        // Tenta converter o JSON recebido para a nova struct com 3 campos
                        match serde_json::from_slice::<SensorData>(&p.payload) {
                            Ok(dados) => {
                                println!("Recebido: Temp: {}, Umid: {}, Press: {}", 
                                    dados.temperatura, dados.umidade, dados.pressao);
                                
                                let mut guard = estado_para_mqtt.lock().unwrap();
                                *guard = dados;
                            },
                            Err(e) => {
                                // Dica de Mentor: É importante logar erros de parse.
                                // Se o JSON vier errado, saberemos o porquê.
                                println!("Erro ao ler JSON: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Erro de conexão MQTT: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // 4. Servidor Web
    let app = Router::new()
        .route("/", get(handler_dashboard))
        .with_state(estado_compartilhado);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Dashboard v2 rodando em http://localhost:3000");
    
    axum::serve(listener, app).await.unwrap();
}

async fn handler_dashboard(State(state): State<SharedState>) -> Html<String> {
    let data = state.lock().unwrap();
    
    // HTML atualizado com 3 Cards usando CSS Flexbox
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
            <head>
                <title>Rusty Weather Station</title>
                <meta http-equiv="refresh" content="5"> <style>
                    body {{ font-family: 'Segoe UI', sans-serif; background-color: #f0f2f5; text-align: center; padding: 20px; }}
                    .container {{ display: flex; justify-content: center; gap: 20px; flex-wrap: wrap; margin-top: 50px; }}
                    .card {{ 
                        background: white; 
                        padding: 20px; 
                        border-radius: 15px; 
                        box-shadow: 0 4px 6px rgba(0,0,0,0.1); 
                        width: 200px;
                    }}
                    .value {{ font-size: 3rem; font-weight: bold; margin: 10px 0; }}
                    .unit {{ font-size: 1rem; color: #666; }}
                    .temp {{ color: #e74c3c; }}
                    .umid {{ color: #3498db; }}
                    .press {{ color: #2ecc71; }}
                </style>
            </head>
            <body>
                <h1>Rusty Weather Dashboard 🦀</h1>
                <p>Monitoramento em Tempo Real via MQTT</p>
                
                <div class="container">
                    <div class="card">
                        <h3>Temperatura</h3>
                        <div class="value temp">{:.1}</div>
                        <div class="unit">°C</div>
                    </div>
                    
                    <div class="card">
                        <h3>Umidade</h3>
                        <div class="value umid">{:.1}</div>
                        <div class="unit">%</div>
                    </div>

                    <div class="card">
                        <h3>Pressão</h3>
                        <div class="value press">{:.1}</div>
                        <div class="unit">hPa</div>
                    </div>
                </div>
                <br>
                <small>Última leitura recebida do ESP32</small>
            </body>
        </html>
        "#,
        data.temperatura,
        data.umidade,
        data.pressao
    );

    Html(html)
}
