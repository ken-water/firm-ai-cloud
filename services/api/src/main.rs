mod alerts;
mod audit;
mod auth;
mod auth_api;
mod cmdb;
mod cockpit;
mod discovery_scheduler_worker;
mod error;
mod iam;
mod monitoring;
mod monitoring_sync_worker;
mod playbooks;
mod secrets;
mod setup;
mod state;
mod streams;
mod tickets;
mod topology;
mod workflow;

use std::net::SocketAddr;

use anyhow::Context;
use axum::{Json, Router, extract::State, http::Method, routing::get};
use common::config::AppConfig;
use common::models::{HealthResponse, PingResponse};
use error::AppResult;
use sqlx::postgres::PgPoolOptions;
use state::{
    AppState, LdapSettings, MonitoringSecretSettings, OidcSettings, WorkflowExecutionSettings,
};
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
        oidc: OidcSettings {
            enabled: config.oidc_enabled,
            authorization_endpoint: config.oidc_authorization_endpoint,
            token_endpoint: config.oidc_token_endpoint,
            userinfo_endpoint: config.oidc_userinfo_endpoint,
            client_id: config.oidc_client_id,
            client_secret: config.oidc_client_secret,
            redirect_uri: config.oidc_redirect_uri,
            scope: config.oidc_scope,
            auto_provision: config.oidc_auto_provision,
            session_ttl_minutes: config.oidc_session_ttl_minutes,
            dev_mode_enabled: config.oidc_dev_mode_enabled,
        },
        ldap: LdapSettings {
            enabled: config.ldap_enabled,
            mode: config.ldap_mode,
            auto_provision: config.ldap_auto_provision,
            dev_users_json: config.ldap_dev_users_json,
            group_role_mapping_json: config.ldap_group_role_mapping_json,
        },
        monitoring_secret: MonitoringSecretSettings {
            encryption_key: config.monitoring_secret_encryption_key,
            inline_policy: config.monitoring_secret_inline_policy,
        },
        workflow_execution: WorkflowExecutionSettings {
            policy_mode: config.workflow_execution_policy_mode,
            allowlist: config.workflow_execution_allowlist,
            sandbox_dir: config.workflow_execution_sandbox_dir,
        },
    };
    auth_api::validate_ldap_group_role_mapping_config(
        state.ldap.group_role_mapping_json.as_deref(),
    )
    .context("invalid AUTH_LDAP_GROUP_ROLE_MAPPING_JSON")?;
    monitoring_sync_worker::start(state.clone());
    discovery_scheduler_worker::start(state.clone());
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
        cmdb_routes = cmdb_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut iam_routes = iam::routes();
    if state.rbac_enabled {
        iam_routes = iam_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut monitoring_routes = monitoring::routes();
    if state.rbac_enabled {
        monitoring_routes = monitoring_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut audit_routes = audit::routes();
    if state.rbac_enabled {
        audit_routes = audit_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut stream_routes = streams::routes();
    if state.rbac_enabled {
        stream_routes = stream_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut setup_routes = setup::routes();
    if state.rbac_enabled {
        setup_routes = setup_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut alert_routes = alerts::routes();
    if state.rbac_enabled {
        alert_routes = alert_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut topology_routes = topology::routes();
    if state.rbac_enabled {
        topology_routes = topology_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut workflow_routes = workflow::routes().merge(playbooks::routes());
    if state.rbac_enabled {
        workflow_routes = workflow_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut ticket_routes = tickets::routes();
    if state.rbac_enabled {
        ticket_routes = ticket_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    let mut ops_routes = cockpit::routes();
    if state.rbac_enabled {
        ops_routes = ops_routes.route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac_guard,
        ));
    }

    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/ping", get(ping_handler))
        .nest("/api/v1/auth", auth_api::routes())
        .nest("/api/v1/cmdb", cmdb_routes)
        .nest("/api/v1/monitoring", monitoring_routes)
        .nest("/api/v1/workflow", workflow_routes)
        .nest("/api/v1", ticket_routes)
        .nest("/api/v1/streams", stream_routes)
        .nest("/api/v1/setup", setup_routes)
        .nest("/api/v1/alerts", alert_routes)
        .nest("/api/v1/topology", topology_routes)
        .nest("/api/v1/ops", ops_routes)
        .nest("/api/v1/iam", iam_routes)
        .nest("/api/v1/audit", audit_routes)
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
