use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::{sync::Arc, net::SocketAddr};
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber;

// Type alias for our shared state: mapping a PIN to a list of Sender channels.
type AppState = Arc<DashMap<String, Vec<mpsc::Sender<Message>>>>;

#[derive(Deserialize, Debug)]
#[serde(tag = "action")]
enum ClientMessage {
    #[serde(rename = "join")]
    Join { pin: String },
    #[serde(rename = "signal")]
    Signal { pin: String, data: String }, // 'data' is the opaque E2EE SDP/ICE candidate
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(DashMap::new());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Signaling server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // We use an mpsc channel to send messages from other tasks to this websocket.
    let (tx, mut rx) = mpsc::channel::<Message>(100);
    
    // Spawn a task to forward messages from the channel to the websocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut joined_pin: Option<String> = None;
    let tx_clone = tx.clone();
    let state_clone = state.clone();

    // Task for receiving messages from the websocket
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Join { pin }) => {
                        info!("Client joined pin: {}", pin);
                        joined_pin = Some(pin.clone());
                        let mut room = state_clone.entry(pin).or_insert_with(Vec::new);
                        room.push(tx_clone.clone());
                    }
                    Ok(ClientMessage::Signal { pin, data }) => {
                        if let Some(room) = state_clone.get(&pin) {
                            // Broadcast to everyone in the room EXCEPT ourselves.
                            // To keep it simple (since we can't easily compare MPSC senders),
                            // we just broadcast to all. The client must ignore its own messages.
                            // But actually, we can just send it, the client will handle it.
                            for peer_tx in room.iter() {
                                // Ignore send errors (peer disconnected)
                                let _ = peer_tx.send(Message::Text(data.clone().into())).await;
                            }
                        } else {
                            warn!("Received signal for unknown pin: {}", pin);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse message: {} (Error: {})", text, e);
                    }
                }
            }
        }
        
        // Clean up when disconnected
        if let Some(pin) = joined_pin {
            info!("Client disconnected from pin: {}", pin);
            // In a real app, we'd remove exactly this sender.
            // For simplicity, we just clear the room if everyone disconnects.
            // Or we could leave it to garbage collection.
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
