use std::net::SocketAddr;

use anyhow::Context;
use axum::{Json, Router, routing::get};
use common::config::AppConfig;
use common::models::{HealthResponse, PingResponse};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env().context("failed to load API config")?;
    init_tracing(&config.log_level);

    let addr: SocketAddr = config.bind_addr().parse().context("invalid bind address")?;

    let app = build_router();
    info!(%addr, "api service starting");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind tcp listener")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("api server exited unexpectedly")?;

    info!("api service stopped");
    Ok(())
}

fn init_tracing(default_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn build_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/ping", get(ping_handler))
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "api",
    })
}

async fn ping_handler() -> Json<PingResponse> {
    Json(PingResponse { message: "pong" })
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        warn!(error = %err, "failed to listen for ctrl-c signal");
    }
}
