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

