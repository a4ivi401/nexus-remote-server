use axum::extract::ws::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use crate::config::Config;
use crate::session::SessionManager;
use crate::turn;
use crate::ws::messages::{ClientMessage, ServerMessage};

pub async fn handle_message(
    msg: ClientMessage,
    session_id: &mut Option<String>,
    tx: &mpsc::Sender<Message>,
    sessions: &Arc<SessionManager>,
    config: &Config,
) {
    match msg {
        ClientMessage::Register => {
            if session_id.is_some() {
                send(tx, ServerMessage::error("already_registered", "Already registered")).await;
                return;
            }
            let id = sessions.register(tx.clone());
            let turn_creds = turn::generate_credentials(config, &id);
            info!(session_id = %id, "Client registered");
            *session_id = Some(id.clone());
            send(
                tx,
                ServerMessage::Registered {
                    session_id: id,
                    turn: turn_creds,
                },
            )
            .await;
        }

        ClientMessage::ConnectRequest { target_id } => {
            let Some(my_id) = session_id.as_ref() else {
                send(tx, ServerMessage::error("not_registered", "Register first")).await;
                return;
            };

            if my_id == &target_id {
                send(tx, ServerMessage::error("invalid_target", "Cannot connect to yourself")).await;
                return;
            }

            if !sessions.session_exists(&target_id) {
                send(
                    tx,
                    ServerMessage::ConnectionRejected {
                        peer_id: target_id,
                        reason: "not_found".into(),
                    },
                )
                .await;
                return;
            }

            if sessions.get_peer(&target_id).is_some() {
                send(
                    tx,
                    ServerMessage::ConnectionRejected {
                        peer_id: target_id,
                        reason: "busy".into(),
                    },
                )
                .await;
                return;
            }

            info!(from = %my_id, to = %target_id, "Connection request");
            let msg = ServerMessage::ConnectionRequest {
                requester_id: my_id.clone(),
            };
            if !send_to(sessions, &target_id, msg).await {
                send(
                    tx,
                    ServerMessage::ConnectionRejected {
                        peer_id: target_id,
                        reason: "delivery_failed".into(),
                    },
                )
                .await;
            }
        }

        ClientMessage::ConnectResponse {
            requester_id,
            accepted,
        } => {
            let Some(my_id) = session_id.as_ref() else {
                send(tx, ServerMessage::error("not_registered", "Register first")).await;
                return;
            };

            if !sessions.session_exists(&requester_id) {
                send(
                    tx,
                    ServerMessage::error("target_not_found", "Requester no longer connected"),
                )
                .await;
                return;
            }

            if accepted {
                sessions.add_connection(my_id, &requester_id);
                info!(a = %my_id, b = %requester_id, "Connection established");

                send(
                    tx,
                    ServerMessage::ConnectionAccepted {
                        peer_id: requester_id.clone(),
                    },
                )
                .await;
                send_to(
                    sessions,
                    &requester_id,
                    ServerMessage::ConnectionAccepted {
                        peer_id: my_id.clone(),
                    },
                )
                .await;
            } else {
                info!(from = %my_id, requester = %requester_id, "Connection rejected");
                send_to(
                    sessions,
                    &requester_id,
                    ServerMessage::ConnectionRejected {
                        peer_id: my_id.clone(),
                        reason: "rejected".into(),
                    },
                )
                .await;
            }
        }

        ClientMessage::Signal { target_id, data } => {
            let Some(my_id) = session_id.as_ref() else {
                send(tx, ServerMessage::error("not_registered", "Register first")).await;
                return;
            };

            let peer = sessions.get_peer(my_id);
            if peer.as_deref() != Some(&target_id) {
                send(
                    tx,
                    ServerMessage::error("not_connected", "Not connected to this peer"),
                )
                .await;
                return;
            }

            send_to(
                sessions,
                &target_id,
                ServerMessage::Signal {
                    from_id: my_id.clone(),
                    data,
                },
            )
            .await;
        }

        ClientMessage::Disconnect { target_id } => {
            let Some(my_id) = session_id.as_ref() else {
                return;
            };

            if let Some(peer_id) = sessions.remove_connection(my_id) {
                if peer_id == target_id {
                    info!(from = %my_id, peer = %peer_id, "Disconnected");
                    send_to(
                        sessions,
                        &peer_id,
                        ServerMessage::PeerDisconnected {
                            peer_id: my_id.clone(),
                        },
                    )
                    .await;
                }
            }
        }

        ClientMessage::Ping => {
            send(tx, ServerMessage::Pong).await;
        }

        ClientMessage::Pong => {}
    }
}

pub async fn handle_disconnect(session_id: &Option<String>, sessions: &Arc<SessionManager>) {
    let Some(id) = session_id else { return };

    info!(session_id = %id, "Client disconnected");
    if let Some(peer_id) = sessions.get_peer(id) {
        sessions.remove_connection(id);
        send_to(
            sessions,
            &peer_id,
            ServerMessage::PeerDisconnected {
                peer_id: id.clone(),
            },
        )
        .await;
    }
    sessions.unregister(id);
}

async fn send(tx: &mpsc::Sender<Message>, msg: ServerMessage) {
    let json = serde_json::to_string(&msg).expect("ServerMessage serialization cannot fail");
    let _ = tx.send(Message::Text(json.into())).await;
}

async fn send_to(sessions: &Arc<SessionManager>, target_id: &str, msg: ServerMessage) -> bool {
    let json = serde_json::to_string(&msg).expect("ServerMessage serialization cannot fail");
    sessions.send_to(target_id, Message::Text(json.into())).await
}
