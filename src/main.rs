use axum::{
    extract::State,
    response::Html,
    routing::get,
    Router,
};
use rumqttc::{MqttOptions, AsyncClient, QoS, Event, Packet};
use serde::Deserialize;
use std::{sync::{Arc, Mutex}, time::Duration};
use std::net::SocketAddr;

// Estrutura que representa os dados que vêm do sensor (JSON)
// O macro #[derive(Deserialize)] permite que o Serde transforme texto JSON nesta struct automaticamente.
#[derive(Deserialize, Debug, Clone, Default)]
struct SensorData {
    temperatura: f64,
}

// O estado compartilhado da nossa aplicação.
// Por que Arc<Mutex<...>>?
// 1. Arc (Atomic Reference Counting): O Rust tem um dono (Owner) único para cada dado.
//    Para compartilhar dados entre threads (Web server e MQTT loop), precisamos de múltiplos "donos".
//    O Arc permite clonar o ponteiro para o dado, mantendo-o vivo enquanto alguém estiver usando.
// 2. Mutex (Mutual Exclusion): O Arc permite leitura compartilhada, mas o Rust proíbe mutação (alteração)
//    concorrente para evitar Data Races. O Mutex garante que apenas UMA thread altere o dado por vez.
type SharedState = Arc<Mutex<SensorData>>;

#[tokio::main]
async fn main() {
    // 1. Inicializar o Estado Compartilhado na memória (começa com 0.0)
    let estado_compartilhado = Arc::new(Mutex::new(SensorData::default()));

    // 2. Configuração do MQTT
    let mut mqttoptions = MqttOptions::new("rust-backend", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    // O AsyncClient é usado para publicar/inscrever. O eventloop é onde recebemos as mensagens.
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Inscreve no tópico. O .await é necessário pois é uma operação de rede (assíncrona).
    client
        .subscribe("sensores/esp32", QoS::AtLeastOnce)
        .await
        .unwrap();

    // 3. Loop MQTT em uma Task separada (Concorrência)
    // O tokio::spawn lança uma "green thread". Ela roda em paralelo ao servidor web.
    // Precisamos de um clone do Arc para passar para dentro dessa nova thread.
    let estado_para_mqtt = estado_compartilhado.clone();
    
    tokio::spawn(async move {
        loop {
            // Aguarda o próximo evento do broker
            match eventloop.poll().await {
                Ok(notification) => {
                    // Verifica se o evento é uma mensagem recebida (Publish)
                    if let Event::Incoming(Packet::Publish(p)) = notification {
                        // Tenta converter os bytes da mensagem (payload) para nossa Struct
                        if let Ok(dados) = serde_json::from_slice::<SensorData>(&p.payload) {
                            println!("Recebido via MQTT: {:?}", dados);
                            
                            // AQUI A MÁGICA DO MUTEX:
                            // .lock() bloqueia o acesso. Se outra thread estiver lendo, esperamos ela terminar.
                            // .unwrap() é usado caso a thread anterior tenha "pânico" (ignorar por enquanto).
                            let mut guard = estado_para_mqtt.lock().unwrap();
                            *guard = dados; // Atualizamos o valor na memória
                            // O "guard" morre aqui (sai de escopo), liberando o Mutex automaticamente.
                        }
                    }
                }
                Err(e) => {
                    println!("Erro no MQTT: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // 4. Configuração do Servidor Web Axum
    // Passamos o estado_compartilhado para o Axum gerenciar.
    let app = Router::new()
        .route("/", get(handler_dashboard))
        .with_state(estado_compartilhado);

    // Define o endereço IP e Porta
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Servidor Web rodando em http://localhost:3000");
    
    // Inicia o servidor
    axum::serve(listener, app).await.unwrap();
}

// 5. Handler da Rota (O que acontece quando você acessa o site)
// State(state): O Axum extrai automaticamente o Arc<Mutex<..>> que passamos no main.
async fn handler_dashboard(State(state): State<SharedState>) -> Html<String> {
    // Bloqueamos o Mutex para leitura.
    let data = state.lock().unwrap();
    
    // Criamos um HTML simples formatando a string com o valor atual
    let html = format!(
        r#"
        <html>
            <head><title>Dashboard IoT Rust</title></head>
            <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                <h1>Monitoramento em Tempo Real</h1>
                <div style="border: 2px solid #333; padding: 20px; display: inline-block; border-radius: 10px;">
                    <h2>Temperatura Atual</h2>
                    <p style="font-size: 4rem; color: #d35400;">{:.1} °C</p>
                </div>
                <p><i>Atualize a página para ver o novo valor.</i></p>
            </body>
        </html>
        "#,
        data.temperatura
    );

    Html(html)
}
