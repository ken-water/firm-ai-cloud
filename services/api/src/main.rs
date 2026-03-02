mod auth;
mod cmdb;
mod error;
mod state;

use std::net::SocketAddr;

use anyhow::Context;
use axum::{Json, Router, extract::State, http::Method, routing::get};
use common::config::AppConfig;
use common::models::{HealthResponse, PingResponse};
use error::AppResult;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env().context("failed to load API config")?;
    init_tracing(&config.log_level);

    let db_pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to postgres")?;

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .context("failed to run database migrations")?;

    let addr: SocketAddr = config.bind_addr().parse().context("invalid bind address")?;
    let state = AppState {
        db: db_pool,
        rbac_enabled: config.rbac_enabled,
    };
    let app = build_router(state);

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

fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let mut cmdb_routes = cmdb::routes();
    if state.rbac_enabled {
        cmdb_routes = cmdb_routes
            .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth::rbac_guard));
    }

    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/ping", get(ping_handler))
        .nest("/api/v1/cmdb", cmdb_routes)
        .with_state(state)
        .layer(cors)
}

async fn health_handler(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await?;

    Ok(Json(HealthResponse {
        status: "ok",
        service: "api",
    }))
}

async fn ping_handler() -> Json<PingResponse> {
    Json(PingResponse { message: "pong" })
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        warn!(error = %err, "failed to listen for ctrl-c signal");
    }
}
