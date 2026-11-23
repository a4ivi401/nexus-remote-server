<div align="center">

# ✨ Nexus Remote Server

**High-performance WebRTC Signaling & TURN Relay Server for Nexus Remote**

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange?style=flat-square)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-compose-2496ED?style=flat-square&logo=docker&logoColor=white)](docker-compose.yml)

**Axum · DashMap · coturn TURN Relay · WebRTC Signaling · Zero-Allocation Routing**

</div>

---

## 📋 Обзор

**Nexus Remote Server** — сверхбыстрый и легковесный Signaling & Relay сервер для приложения удаленного рабочего стола Nexus Remote (аналог AnyDesk / TeamViewer).

### Ключевые возможности:
- **Уникальные 6-значные Session ID**: автоматическая генерация и коллизионная проверка при регистрации клиента.
- **Интерактивный брокеринг подключений**: поддержка запросов на подключение (`connect_request`), подтверждения (`connect_response` / accept / reject) и уведомлений об отключении.
- **Интеграция с coturn (TURN/STUN)**: динамическая генерация time-limited HMAC-SHA1 учетных данных (RFC 5766 / RFC 5389) для автоматического fallback на релей, если прямое P2P-соединение невозможно из-за Symmetric NAT / Firewall.
- **Lock-free State Management**: многопоточное управление сессиями через `DashMap` и каналы `tokio::sync::mpsc`.
- **E2E маршрутизация сигналов**: прозрачная пересылка SDP offer/answer и ICE candidates в реальном времени.
- **Healthcheck & Метрики**: HTTP эндпоинт `/health` с количеством активных сессий и соединений.
- **Docker Compose**: готовая конфигурация для развертывания сигнального сервера и coturn в изолированных контейнерах.

---

## 🏗️ Архитектура

```
                           ┌────────────────────────┐
                           │   Nexus Remote Server  │
                           │     (Axum + Tokio)     │
                           └───────────┬────────────┘
                                       │ WebSocket (/ws)
                ┌──────────────────────┴──────────────────────┐
                ▼                                             ▼
       ┌─────────────────┐                           ┌─────────────────┐
       │  Client A (Host) │                           │ Client B (Viewer)│
       │  Session: 849201 │                           │  Session: 193842 │
       └────────┬────────┘                           └────────┬────────┘
                │                                             │
                │ 1. P2P Direct WebRTC (STUN / Host / Srflx)  │
                ├─────────────────────────────────────────────┤
                │                                             │
                │ 2. Fallback: TURN Relay (coturn, UDP/TCP)   │
                └──────────────────────┬──────────────────────┘
                                       ▼
                             ┌───────────────────┐
                             │   coturn Server   │
                             │ (Port 3478 / UDP) │
                             └───────────────────┘
```

### Стек технологий

| Компонент | Назначение |
|---|---|
| [axum 0.8](https://docs.rs/axum/) | Асинхронный HTTP/WebSocket сервер |
| [tokio](https://tokio.rs/) | Асинхронный рантайм |
| [dashmap](https://docs.rs/dashmap/) | Высокопроизводительный lock-free concurrent hash map |
| [coturn](https://github.com/coturn/coturn) | Высокопроизводительный STUN / TURN сервер |
| [hmac](https://docs.rs/hmac/) + [sha1](https://docs.rs/sha1/) | Генерация временных TURN-токенов |
| [tracing](https://docs.rs/tracing/) | Структурированное асинхронное логирование |

---

## 📡 Протокол сигналинга (JSON over WebSocket)

### 1. Сообщения Клиент → Сервер

| Тип (`type`) | Поля | Описание |
|---|---|---|
| `register` | — | Зарегистрироваться на сервере и получить Session ID + TURN creds |
| `connect_request` | `target_id: String` | Запросить подключение к удаленному клиенту |
| `connect_response` | `requester_id: String`, `accepted: bool` | Принять или отклонить входящий запрос |
| `signal` | `target_id: String`, `data: String` | Переслать WebRTC SDP / ICE payload пиру |
| `disconnect` | `target_id: String` | Завершить сессию с пиром |
| `ping` | — | Heartbeat ping |
| `pong` | — | Heartbeat pong |

### 2. Сообщения Сервер → Клиент

| Тип (`type`) | Поля | Описание |
|---|---|---|
| `registered` | `session_id: String`, `turn: TurnCredentials` | Успешная регистрация и выданные STUN/TURN реквизиты |
| `connection_request` | `requester_id: String` | Входящий запрос на подключение от другого клиента |
| `connection_accepted` | `peer_id: String` | Соединение подтверждено, можно начинать обмен SDP/ICE |
| `connection_rejected` | `peer_id: String`, `reason: String` | Запрос отклонен (`"rejected"`, `"busy"`, `"not_found"`) |
| `signal` | `from_id: String`, `data: String` | Входящий WebRTC сигнал от пира |
| `peer_disconnected` | `peer_id: String` | Пир разорвал соединение |
| `error` | `code: String`, `message: String` | Ошибка обработки запроса |
| `ping` / `pong` | — | Heartbeat сообщения |

---

## ⚙️ Конфигурация (.env)

| Переменная | По умолчанию | Описание |
|---|---|---|
| `HOST` | `0.0.0.0` | IP-адрес для привязки сервера |
| `PORT` | `3000` | Порт HTTP/WebSocket сервера |
| `RUST_LOG` | `info` | Уровень логирования (`trace`, `debug`, `info`, `warn`, `error`) |
| `TURN_SECRET` | `nexus-dev-secret` | Общий секрет (HMAC-SHA1) с coturn |
| `TURN_HOST` | `127.0.0.1` | Публичный IP или домен TURN сервера |
| `TURN_PORT` | `3478` | Порт STUN/TURN сервера |
| `TURN_TTL` | `86400` | Время жизни TURN-учетных данных (в секундах) |

---

## 🚀 Запуск и Развертывание

### Локальный запуск (Cargo)

```bash
# 1. Клонирование репозитория
git clone https://github.com/a4ivi4/nexus-remote-server.git
cd nexus-remote-server

# 2. Настройка переменных окружения
cp .env.example .env

# 3. Запуск тестов
cargo test

# 4. Запуск сервера в Release режиме
cargo run --release
```

### Развертывание через Docker Compose

```bash
# Сборка и запуск сигнального сервера и coturn
docker compose up -d --build

# Просмотр логов
docker compose logs -f

# Проверка healthcheck
curl http://localhost:3000/health
```

---

## 🔒 Безопасность и End-to-End шифрование

- **Zero Data Retention**: Сигнальный сервер не сохраняет историю сообщений и передаваемые медиа-данные.
- **E2EE Payload**: Поле `data` в сообщениях `signal` передает непрозрачный для сервера payload (включая зашифрованные метаданные и WebRTC SDP).
- **Time-Limited TURN Tokens**: Учетные данные к релею coturn генерируются индивидуально для каждой сессии с ограниченным сроком действия (TTL).

---

## 📄 Лицензия

MIT © [a4ivi4](https://github.com/a4ivi4)
