# 📡 Nexus Remote Signaling Protocol Specification (v2)

Данная спецификация описывает стандарт взаимодействия между клиентом (**Nexus Remote Client**) и сигнальным сервером (**Nexus Remote Server**).

---

## 1. Общие принципы

- **Транспорт**: WebSocket (`ws://` для локальной разработки, `wss://` для продакшена).
- **Эндпоинт**: `/ws` (например, `ws://127.0.0.1:3000/ws`).
- **Формат сообщений**: Текстовые фреймы WebSocket, содержащие валидный UTF-8 **JSON**.
- **Дискриминатор сообщений**: Каждое сообщение обязательно содержит строковое поле `"type"`.
- **E2EE / Zero-Knowledge Relay**: Сервер не анализирует и не парсит содержимое поля `data` в сообщениях типа `signal` — клиент может передавать туда как открытый JSON (SDP / ICE), так и зашифрованный Base64 payload.

---

## 2. Диаграмма последовательности (Signaling Flow)

```text
 Client A (Viewer / Управление)               Signaling Server             Client B (Host / Трансляция)
        │                                           │                                   │
        ├─────────── WebSocket Connect ────────────►│                                   │
        ├─────────── {"type": "register"} ─────────►│                                   │
        │◄── {"type": "registered", id: "102938"} ──┤                                   │
        │                                           │                                   │
        │                                           │◄────────── WebSocket Connect ─────┤
        │                                           │◄────────── {"type": "register"} ──┤
        │                                           ├── {"type": "registered", id: "849201"} ─►│
        │                                           │                                   │
        │─── {"type": "connect_request", ──────────►│                                   │
        │     "target_id": "849201"}                ├─── {"type": "connection_request", ─►│
        │                                           │     "requester_id": "102938"}     │
        │                                           │                                   │
        │                                           │◄─── {"type": "connect_response", ─┤
        │                                           │     "requester_id": "102938",     │
        │                                           │     "accepted": true}             │
        │◄── {"type": "connection_accepted", ───────┤                                   │
        │     "peer_id": "849201"}                  ├─── {"type": "connection_accepted", ►│
        │                                           │     "peer_id": "102938"}          │
        │                                           │                                   │
        │   ═════════════════════ WebRTC Сигналинг (SDP & ICE) ═════════════════════    │
        │                                           │                                   │
        │─── {"type": "signal", target_id: "849201",►│                                   │
        │     "data": "<SDP Offer>"}                ├─── {"type": "signal", from_id: ...►│
        │                                           │                                   │
        │                                           │◄─── {"type": "signal", target_id: ┤
        │◄── {"type": "signal", from_id: ... ───────┤     "data": "<SDP Answer>"}       │
        │                                           │                                   │
        │─── {"type": "signal", data: "<ICE>"} ────►│─── {"type": "signal", ...} ──────►│
        │◄── {"type": "signal", ...} ───────────────│◄─── {"type": "signal", data: ...} ┤
        │                                           │                                   │
        │   ════════════════ Прямое P2P соединение (или через coturn) ═══════════════   │
        │◄─────────────────────────── WebRTC Video Stream ─────────────────────────────►│
        │                                           │                                   │
        │─── {"type": "disconnect", ...} ──────────►│                                   │
        │                                           ├─── {"type": "peer_disconnected"} ─►│
```

---

## 3. Спецификация сообщений: Клиент $\rightarrow$ Сервер

### 3.1. Регистрация (`register`)
Отправляется клиентом сразу после установки WebSocket соединения для получения уникального 6-значного ID и реквизитов TURN.
```json
{
  "type": "register"
}
```

### 3.2. Запрос на подключение (`connect_request`)
Отправляется клиентом, желающим подключиться к удаленному компьютеру.
```json
{
  "type": "connect_request",
  "target_id": "849201"
}
```

### 3.3. Ответ на входящее подключение (`connect_response`)
Отправляется хостом в ответ на входящий `connection_request`.
```json
{
  "type": "connect_response",
  "requester_id": "102938",
  "accepted": true
}
```
*Если `accepted: false`, сервер уведомит инициатора сообщением `connection_rejected`.*

### 3.4. Пересылка WebRTC сигнала (`signal`)
Используется для обмена SDP Offer, SDP Answer и ICE Candidates.
```json
{
  "type": "signal",
  "target_id": "849201",
  "data": "{\"sdp\": \"v=0...\", \"type\": \"offer\"}"
}
```

### 3.5. Завершение сессии (`disconnect`)
Штатное завершение сессии с пиром.
```json
{
  "type": "disconnect",
  "target_id": "849201"
}
```

### 3.6. Heartbeat (`ping` / `pong`)
Рекомендуется отправлять каждые 20–30 секунд для поддержания активности TCP-сокета через NAT/прокси.
```json
{
  "type": "ping"
}
```

---

## 4. Спецификация сообщений: Сервер $\rightarrow$ Клиент

### 4.1. Успешная регистрация (`registered`)
Возвращает выданный 6-значный ID и конфигурацию STUN/TURN серверов.
```json
{
  "type": "registered",
  "session_id": "849201",
  "turn": {
    "urls": [
      "stun:127.0.0.1:3478",
      "turn:127.0.0.1:3478?transport=udp",
      "turn:127.0.0.1:3478?transport=tcp"
    ],
    "username": "1758402265:849201",
    "credential": "dGVzdC1jcmVkZW50aWFs"
  }
}
```

### 4.2. Входящий запрос на подключение (`connection_request`)
Приходит хосту, когда кто-то ввел его ID и нажал «Подключиться».
```json
{
  "type": "connection_request",
  "requester_id": "102938"
}
```

### 4.3. Соединение подтверждено (`connection_accepted`)
Приходит **обоим клиентам** после того, как хост подтвердил запрос. Сигнал к началу WebRTC Handshake.
```json
{
  "type": "connection_accepted",
  "peer_id": "849201"
}
```

### 4.4. Соединение отклонено (`connection_rejected`)
Приходит инициатору, если подключение невозможно.
```json
{
  "type": "connection_rejected",
  "peer_id": "849201",
  "reason": "rejected"
}
```
**Возможные значения `reason`:**
- `"rejected"` — пользователь на удаленном ПК отклонил запрос.
- `"not_found"` — сессия с таким ID не существует или отключена.
- `"busy"` — удаленный ПК уже находится в активной сессии с другим пользователем.

### 4.5. Входящий WebRTC сигнал (`signal`)
Приходит при получении SDP или ICE-кандидата от удаленного пира.
```json
{
  "type": "signal",
  "from_id": "849201",
  "data": "{\"sdp\": \"v=0...\", \"type\": \"offer\"}"
}
```

### 4.6. Пир отключился (`peer_disconnected`)
Приходит, если удаленный клиент нажал `disconnect` или его WebSocket соединение оборвалось.
```json
{
  "type": "peer_disconnected",
  "peer_id": "849201"
}
```

### 4.7. Ошибка обработки (`error`)
```json
{
  "type": "error",
  "code": "not_registered",
  "message": "Must register first"
}
```
**Коды ошибок (`code`):**
- `not_registered` — отправка команд до вызова `register`.
- `already_registered` — повторный вызов `register` на том же сокете.
- `invalid_target` — попытка подключиться к собственному ID.
- `not_connected` — отправка `signal` или `disconnect` без установленного соединения.
- `invalid_message` — некорректный JSON синтаксис.

---

## 5. Использование STUN / TURN на клиенте (WebRTC)

Объект `turn`, полученный в сообщении `registered`, напрямую мапится в стандартную конфигурацию WebRTC `RTCConfiguration` (в браузере, Tauri, GStreamer `webrtcbin` или `webrtc-rs`):

### В Rust (`webrtc-rs`):
```rust
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;

let ice_servers = vec![
    RTCIceServer {
        urls: turn_creds.urls,
        username: turn_creds.username,
        credential: turn_creds.credential,
        ..Default::default()
    }
];

let config = RTCConfiguration {
    ice_servers,
    ..Default::default()
};
```

---

## 6. Rust-типы для клиента (Serde DTOs)

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "register")]
    Register,

    #[serde(rename = "connect_request")]
    ConnectRequest { target_id: String },

    #[serde(rename = "connect_response")]
    ConnectResponse {
        requester_id: String,
        accepted: bool,
    },

    #[serde(rename = "signal")]
    Signal { target_id: String, data: String },

    #[serde(rename = "disconnect")]
    Disconnect { target_id: String },

    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "ping")]
    Ping,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TurnCredentials {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "registered")]
    Registered {
        session_id: String,
        turn: TurnCredentials,
    },

    #[serde(rename = "connection_request")]
    ConnectionRequest { requester_id: String },

    #[serde(rename = "connection_accepted")]
    ConnectionAccepted { peer_id: String },

    #[serde(rename = "connection_rejected")]
    ConnectionRejected { peer_id: String, reason: String },

    #[serde(rename = "signal")]
    Signal { from_id: String, data: String },

    #[serde(rename = "peer_disconnected")]
    PeerDisconnected { peer_id: String },

    #[serde(rename = "error")]
    Error { code: String, message: String },

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "pong")]
    Pong,
}
```
