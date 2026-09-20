pub mod handler;
pub mod messages;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

use crate::config::Config;
use crate::session::SessionManager;
use messages::ClientMessage;

pub type AppState = Arc<SharedState>;

pub struct SharedState {
    pub sessions: Arc<SessionManager>,
    pub config: Config,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut session_id: Option<String> = None;
    let sessions = state.sessions.clone();
    let config = state.config.clone();
    let tx_clone = tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            handler::handle_message(
                                client_msg,
                                &mut session_id,
                                &tx_clone,
                                &sessions,
                                &config,
                            )
                            .await;
                        }
                        Err(e) => {
                            warn!("Invalid message: {e}");
                            let err_msg = messages::ServerMessage::error(
                                "invalid_message",
                                &format!("Failed to parse: {e}"),
                            );
                            let json = serde_json::to_string(&err_msg).unwrap();
                            let _ = tx_clone.send(Message::Text(json.into())).await;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        handler::handle_disconnect(&session_id, &sessions).await;
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };
}
