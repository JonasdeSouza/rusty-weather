# Rusty Weather Station 🦀

Um dashboard IoT em tempo real desenvolvido em **Rust**, focado em alta performance e segurança de memória. O sistema atua como um backend híbrido, processando mensagens MQTT de sensores (simulando um ESP32) e servindo uma interface Web HTTP simultaneamente.

## 🚀 Sobre o Projeto

Este projeto foi desenvolvido como um estudo prático de conceitos avançados de Rust e Engenharia de Software, incluindo:
- **Concorrência Assíncrona:** Uso de Tasks para processar I/O sem bloquear a CPU.
- **Ownership & Borrowing:** Gerenciamento seguro de memória sem Garbage Collector.
- **Estado Compartilhado:** Sincronização segura entre threads usando `Arc<Mutex>`.

## 🛠️ Stack Tecnológica

As bibliotecas (crates) mais modernas do ecossistema Rust foram utilizadas:

- **[Tokio](https://tokio.rs/):** Runtime assíncrono (o padrão da indústria).
- **[Axum](https://github.com/tokio-rs/axum):** Framework Web ergonômico e modular.
- **[Rumqttc](https://github.com/bytebeamio/rumqtt):** Cliente MQTT leve e robusto.
- **[Serde](https://serde.rs/):** Framework de serialização/deserialização de alta performance.

## 📡 Modelo de Dados (Protocolo JSON)

O sistema espera receber payloads no formato JSON no tópico `sensores/esp32`.
A estrutura rígida de tipos do Rust garante que apenas mensagens válidas sejam processadas.

**Exemplo de Payload Válido:**

```json
{
  "temperatura": 25.5,
  "umidade": 60.0,
  "pressao": 1013.2
}
```

## ⚙️ Pré-requisitos (Linux/Ubuntu)

Certifique-se de ter as ferramentas de build e o Broker MQTT instalados:
```Bash

# Instala compiladores e o Broker Mosquitto
sudo apt update && sudo apt install build-essential mosquitto mosquitto-clients -y

# Instala o Rust (caso não tenha)
curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
```

## ▶️ Como Rodar

  Clone o repositório:
  
```bash
git clone https://github.com/JonasdeSouza/rusty-weather.git
cd rusty-weather
```

Inicie o Servidor:

```bash
    cargo run
```
  O servidor iniciará em http://localhost:3000 e conectará ao broker MQTT local na porta 1883.

## 🧪 Como Testar (Simulação)

Com o servidor rodando, abra outro terminal para simular um sensor ESP32 enviando dados via mosquitto_pub:
```bash

mosquitto_pub -h localhost -t sensores/esp32 -m '{"temperatura": 28.5, "umidade": 62.0, "pressao": 1013.5}'
```
Acesse http://localhost:3000 e veja os cards atualizarem instantaneamente.

