use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::ws::{self, AppState};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    active_sessions: usize,
    active_connections: usize,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok",
        active_sessions: state.sessions.active_sessions(),
        active_connections: state.sessions.active_connections(),
    };
    (StatusCode::OK, Json(response))
}
