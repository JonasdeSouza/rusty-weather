use axum::{
    extract::State,
    response::Html,
    routing::get,
    Router,
};
use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use serde::Deserialize;
use std::{sync::{Arc, Mutex}, time::Duration};
use chrono::Local; // Biblioteca para data/hora

// Dados brutos que vêm do sensor
#[derive(Deserialize, Debug, Clone, Copy)]
struct SensorData {
    temperatura: f64,
    umidade: f64,
    pressao: f64,
}

// Estrutura interna para guardar o dado + a hora que ele chegou
#[derive(Debug, Clone)]
struct Registro {
    dados: SensorData,
    horario: String,
}

// O estado agora é uma LISTA (Vector) de registros
// Usamos VecDeque seria mais eficiente, mas Vec é mais simples para aprender
type SharedState = Arc<Mutex<Vec<Registro>>>;

#[tokio::main]
async fn main() {
    // 1. Inicializa o Estado como uma lista vazia
    let estado_compartilhado = Arc::new(Mutex::new(Vec::new()));

    // 2. Configuração MQTT
    let mut mqttoptions = MqttOptions::new("rust-dashboard-history", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    client
        .subscribe("sensores/esp32", QoS::AtLeastOnce)
        .await
        .unwrap();

    // 3. Loop MQTT
    let estado_para_mqtt = estado_compartilhado.clone();
    
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let Event::Incoming(Packet::Publish(p)) = notification {
                        if let Ok(dados_sensor) = serde_json::from_slice::<SensorData>(&p.payload) {
                            println!("Recebido: {:?}", dados_sensor);
                            
                            // Pega a hora atual do sistema formatada
                            let agora = Local::now().format("%H:%M:%S").to_string();
                            
                            let novo_registro = Registro {
                                dados: dados_sensor,
                                horario: agora,
                            };

                            let mut history = estado_para_mqtt.lock().unwrap();
                            history.push(novo_registro);

                            // LÓGICA DE LIMPEZA: Mantém apenas os últimos 10 registros
                            if history.len() > 10 {
                                history.remove(0); // Remove o mais antigo
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Erro MQTT: {:?}", e);
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
    println!("Dashboard com Histórico rodando em http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn handler_dashboard(State(state): State<SharedState>) -> Html<String> {
    let history = state.lock().unwrap();

    // Pega o registro mais recente (o último da lista), ou usa valores zerados se estiver vazio
    let atual = history.last().cloned().unwrap_or(Registro {
        dados: SensorData { temperatura: 0.0, umidade: 0.0, pressao: 0.0 },
        horario: "--:--:--".to_string(),
    });

    // Gera as linhas da tabela (HTML) iterando sobre o histórico INVERSO (mais novo primeiro)
    let mut linhas_tabela = String::new();
    for reg in history.iter().rev() {
        linhas_tabela.push_str(&format!(
            "<tr>
                <td>{}</td>
                <td>{:.1} °C</td>
                <td>{:.1} %</td>
                <td>{:.1} hPa</td>
            </tr>",
            reg.horario, reg.dados.temperatura, reg.dados.umidade, reg.dados.pressao
        ));
    }

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Rusty Weather Station</title>
            <meta http-equiv="refresh" content="3">
            <style>
                body {{ font-family: sans-serif; background: #f4f4f9; padding: 20px; text-align: center; }}
                .cards {{ display: flex; justify-content: center; gap: 20px; margin-bottom: 40px; }}
                .card {{ background: white; padding: 20px; border-radius: 10px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); width: 180px; }}
                .val {{ font-size: 2.5rem; font-weight: bold; margin: 10px 0; }}
                .ts {{ color: #888; margin-bottom: 20px; }}
                
                table {{ margin: 0 auto; border-collapse: collapse; width: 80%; max-width: 600px; background: white; }}
                th, td {{ padding: 12px; border-bottom: 1px solid #ddd; text-align: center; }}
                th {{ background-color: #333; color: white; }}
                tr:nth-child(even) {{ background-color: #f9f9f9; }}
            </style>
        </head>
        <body>
            <h1>Rusty Weather Dashboard 🦀</h1>
            <div class="ts">Última atualização: <strong>{}</strong></div>

            <div class="cards">
                <div class="card"><div style="color: #e74c3c">Temp</div><div class="val">{:.1}</div><div>°C</div></div>
                <div class="card"><div style="color: #3498db">Umid</div><div class="val">{:.1}</div><div>%</div></div>
                <div class="card"><div style="color: #2ecc71">Press</div><div class="val">{:.1}</div><div>hPa</div></div>
            </div>

            <h3>Histórico Recente (Últimas 10 leituras)</h3>
            <table>
                <thead>
                    <tr>
                        <th>Horário</th>
                        <th>Temp</th>
                        <th>Umidade</th>
                        <th>Pressão</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </body>
        </html>
        "#,
        atual.horario,
        atual.dados.temperatura,
        atual.dados.umidade,
        atual.dados.pressao,
        linhas_tabela
    );

    Html(html)
}
