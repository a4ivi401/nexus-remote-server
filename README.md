<div align="center">

# ✨ Nexus Remote Server

**High-performance WebSocket signaling server for Nexus Remote**

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange?style=flat-square)](https://www.rust-lang.org/)

**Axum · DashMap · E2EE Routing · High RPS**

</div>

---

## 📋 Обзор

**Nexus Remote Server** — это сверхбыстрый и легковесный Signaling-сервер для инфраструктуры Nexus Remote.
Его единственная задача — соединить два клиента (Host и Viewer) и позволить им обменяться зашифрованными WebRTC (SDP и ICE) пакетами для установки прямого P2P соединения. 

Сервер работает как "слепой почтальон": он не знает, что находится внутри пакетов, и не может расшифровать трафик.

## 🏗️ Архитектура

- **Фреймворк**: Сервер построен на базе современного асинхронного веб-фреймворка [Axum](https://docs.rs/axum/).
- **Хранилище стейта**: Для хранения ожидающих соединений используется lock-free структура [DashMap](https://docs.rs/dashmap/), что позволяет серверу выдерживать десятки тысяч подключений без просадок.
- **Протокол**: Клиенты связываются с сервером по протоколу WebSocket.

### Стек технологий

| Компонент | Библиотека |
|-----------|-----------|
| Web-фреймворк | [axum](https://docs.rs/axum/) |
| Асинхронность | [tokio](https://tokio.rs/) |
| In-Memory Cache | [dashmap](https://docs.rs/dashmap/) |
| Логирование | [tracing](https://docs.rs/tracing/) |

## 🚀 Быстрый старт

### Сборка и Запуск

```bash
# Клонирование
git clone https://github.com/a4ivi4/nexus-remote-server.git
cd nexus-remote-server

# Запуск сервера
cargo run --release
```

По умолчанию сервер запустится на `127.0.0.1:3000`.

### Маршруты

- `GET /ws` — Основной WebSocket эндпоинт для подключения клиентов.
- `GET /health` — HTTP эндпоинт для проверки статуса сервера (используется для мониторинга или Load Balancer'ов).

## 🔒 Безопасность (E2E Routing)

- **Отсутствие хранения данных**: Сервер хранит сообщения в оперативной памяти (в `DashMap`) максимум на пару секунд, пока второй клиент не заберет их.
- **Полное шифрование**: Payload пакетов (`data`), которыми обмениваются клиенты, зашифрован ChaCha20-Poly1305. Сервер видит только Base64 строку и PIN-код комнаты.

## 📄 Лицензия

MIT © [a4ivi4](https://github.com/a4ivi4)
