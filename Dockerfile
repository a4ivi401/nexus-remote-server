# Stage 1: Builder
FROM rust:1.85-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy dependency definitions
COPY Cargo.toml Cargo.lock ./

# Create skeleton source tree for caching dependencies
RUN mkdir -p src/ws src/session && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub mod ws; pub mod session; pub mod config; pub mod server; pub mod turn;" > src/lib.rs && \
    echo "pub struct Config;" > src/config.rs && \
    echo "pub fn create_router() {}" > src/server.rs && \
    echo "pub struct SessionManager;" > src/session/mod.rs && \
    echo "pub struct TurnCredentials;" > src/turn.rs && \
    echo "pub struct SharedState;" > src/ws/mod.rs && \
    echo "pub enum ClientMessage {}" > src/ws/messages.rs && \
    echo "pub async fn handle_socket() {}" > src/ws/handler.rs

RUN cargo build --release || true
RUN rm -rf src

# Copy real source code
COPY src ./src

# Touch source files to ensure cargo rebuilds the binary
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/nexus-remote-server /app/nexus-remote-server

ENV HOST=0.0.0.0
ENV PORT=3000
ENV RUST_LOG=info

EXPOSE 3000

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/app/nexus-remote-server"]
