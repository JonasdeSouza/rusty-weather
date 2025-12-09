# Rusty Weather Station 🦀

> **Status:** 🟢 Online no Render | 🦀 Backend em Rust | 📡 Protocolo MQTT

Um dashboard IoT em tempo real desenvolvido em **Rust**, focado em alta performance e segurança de memória. O sistema atua como um backend híbrido, processando mensagens MQTT de sensores (simulando um ESP32) e servindo uma interface Web HTTP simultaneamente.

## 🔗 Links Rápidos
* **Dashboard Online:** [Insira_aqui_seu_link_do_render] (Ex: https://rusty-weather.onrender.com)
* **Documentação Técnica:** [Link_para_sua_Wiki]

## 🛠️ Stack Tecnológica
* **[Tokio](https://tokio.rs/):** Runtime assíncrono.
* **[Axum](https://github.com/tokio-rs/axum):** Framework Web (Porta 3000).
* **[Rumqttc](https://github.com/bytebeamio/rumqtt):** Cliente MQTT (Porta 1883).
* **[Serde](https://serde.rs/):** Serialização JSON segura.

## 📡 Arquitetura de Conexão

O sistema escuta mensagens em um Broker Público. Qualquer dispositivo (ESP32 ou Terminal) pode enviar dados seguindo estes parâmetros:

| Parâmetro | Valor |
| :--- | :--- |
| **Broker Host** | `test.mosquitto.org` |
| **Porta MQTT** | `1883` |
| **Tópico** | `sensores/rusty_weather/unb_211068459` |
| **Formato** | JSON (`temperatura`, `umidade`, `pressao`) |
