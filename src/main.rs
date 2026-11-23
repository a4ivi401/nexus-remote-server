use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use nexus_remote_server::config::Config;
use nexus_remote_server::server;
use nexus_remote_server::session::SessionManager;
use nexus_remote_server::ws::SharedState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let addr = config.socket_addr();
    let sessions = SessionManager::new();

    let state = Arc::new(SharedState {
        sessions,
        config,
    });

    let app = server::create_router(state);

    info!("Signaling server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
