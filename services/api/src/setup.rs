use std::{env, time::Duration};

use axum::{Json, Router, extract::State, routing::get};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::{net::TcpStream, time::timeout};

use crate::{error::AppResult, state::AppState};

const DEFAULT_WEB_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_API_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_REDIS_ADDR: &str = "127.0.0.1:6379";
const DEFAULT_OPENSEARCH_ADDR: &str = "127.0.0.1:9200";
const DEFAULT_MINIO_ADDR: &str = "127.0.0.1:9000";
const DEFAULT_ZABBIX_ADDR: &str = "127.0.0.1:10051";
const TCP_CHECK_TIMEOUT_MS: u64 = 900;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/preflight", get(get_setup_preflight))
        .route("/checklist", get(get_setup_checklist))
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SetupCheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
struct SetupCheckItem {
    key: String,
    title: String,
    status: SetupCheckStatus,
    critical: bool,
    message: String,
    remediation: String,
}

#[derive(Debug, Serialize)]
struct SetupSummary {
    total: usize,
    passed: usize,
    warned: usize,
    failed: usize,
    critical_failed: usize,
    ready: bool,
}

#[derive(Debug, Serialize)]
struct SetupChecklistResponse {
    generated_at: DateTime<Utc>,
    category: String,
    summary: SetupSummary,
    checks: Vec<SetupCheckItem>,
}

async fn get_setup_preflight(
    State(state): State<AppState>,
) -> AppResult<Json<SetupChecklistResponse>> {
    let mut checks = Vec::new();

    checks.push(check_database(state.db.clone()).await);
    checks.push(check_rbac_mode(state.rbac_enabled));
    checks.push(check_oidc_settings(&state));
    checks.push(check_monitoring_secret_settings(&state));
    checks.push(check_workflow_policy_mode(&state));

    Ok(Json(build_response("preflight", checks)))
}

async fn get_setup_checklist(
    State(state): State<AppState>,
) -> AppResult<Json<SetupChecklistResponse>> {
    let mut checks = Vec::new();

    checks.push(SetupCheckItem {
        key: "api-service".to_string(),
        title: "API Service".to_string(),
        status: SetupCheckStatus::Pass,
        critical: true,
        message: "API endpoint is reachable because checklist request succeeded.".to_string(),
        remediation: "If this check fails in other environments, start API service and verify API_HOST/API_PORT.".to_string(),
    });

    checks.push(check_database(state.db.clone()).await.with_key("database"));

    checks.push(
        tcp_endpoint_check(
            "web-console",
            "Web Console",
            &read_addr("SETUP_WEB_ADDR", DEFAULT_WEB_ADDR),
            true,
            "Ensure web console is running (for local stack: bash scripts/install.sh or docker compose up web).",
        )
        .await,
    );

    checks.push(
        tcp_endpoint_check(
            "redis",
            "Redis",
            &read_addr("SETUP_REDIS_ADDR", DEFAULT_REDIS_ADDR),
            false,
            "Ensure redis service is healthy and accessible from API host.",
        )
        .await,
    );

    checks.push(
        tcp_endpoint_check(
            "opensearch",
            "OpenSearch",
            &read_addr("SETUP_OPENSEARCH_ADDR", DEFAULT_OPENSEARCH_ADDR),
            false,
            "Ensure opensearch service is healthy and accessible from API host.",
        )
        .await,
    );

    checks.push(
        tcp_endpoint_check(
            "minio",
            "MinIO",
            &read_addr("SETUP_MINIO_ADDR", DEFAULT_MINIO_ADDR),
            false,
            "Ensure minio service is healthy and accessible from API host.",
        )
        .await,
    );

    checks.push(
        tcp_endpoint_check(
            "zabbix-server",
            "Zabbix Server",
            &read_addr("SETUP_ZABBIX_ADDR", DEFAULT_ZABBIX_ADDR),
            false,
            "Ensure zabbix server is running and the trapper/listener port is reachable.",
        )
        .await,
    );

    checks.push(check_monitoring_source_seed(state.db.clone()).await);
    checks.push(check_alert_policy_templates(state.db.clone()).await);
    checks.push(
        tcp_endpoint_check(
            "api-port",
            "API Port",
            &read_addr("SETUP_API_ADDR", DEFAULT_API_ADDR),
            true,
            "Ensure API bind address is reachable on the expected host and port.",
        )
        .await,
    );

    Ok(Json(build_response("integration_checklist", checks)))
}

fn build_response(category: &str, checks: Vec<SetupCheckItem>) -> SetupChecklistResponse {
    let mut passed = 0_usize;
    let mut warned = 0_usize;
    let mut failed = 0_usize;
    let mut critical_failed = 0_usize;

    for item in &checks {
        match item.status {
            SetupCheckStatus::Pass => passed += 1,
            SetupCheckStatus::Warn => warned += 1,
            SetupCheckStatus::Fail => {
                failed += 1;
                if item.critical {
                    critical_failed += 1;
                }
            }
        }
    }

    let summary = SetupSummary {
        total: checks.len(),
        passed,
        warned,
        failed,
        critical_failed,
        ready: critical_failed == 0,
    };

    SetupChecklistResponse {
        generated_at: Utc::now(),
        category: category.to_string(),
        summary,
        checks,
    }
}

async fn check_database(db: sqlx::PgPool) -> SetupCheckItem {
    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&db).await {
        Ok(_) => SetupCheckItem {
            key: "database".to_string(),
            title: "PostgreSQL".to_string(),
            status: SetupCheckStatus::Pass,
            critical: true,
            message: "PostgreSQL connectivity check passed.".to_string(),
            remediation:
                "If this check fails, verify DATABASE_URL and database service health before proceeding."
                    .to_string(),
        },
        Err(err) => SetupCheckItem {
            key: "database".to_string(),
            title: "PostgreSQL".to_string(),
            status: SetupCheckStatus::Fail,
            critical: true,
            message: format!("PostgreSQL check failed: {err}"),
            remediation:
                "Fix DATABASE_URL, start postgres, and rerun setup checklist until this check passes."
                    .to_string(),
        },
    }
}

fn check_rbac_mode(rbac_enabled: bool) -> SetupCheckItem {
    if rbac_enabled {
        SetupCheckItem {
            key: "rbac".to_string(),
            title: "RBAC Guard".to_string(),
            status: SetupCheckStatus::Pass,
            critical: true,
            message: "RBAC guard is enabled (AUTH_RBAC_ENABLED=true).".to_string(),
            remediation:
                "Keep RBAC enabled for production; disable only for local debugging sessions."
                    .to_string(),
        }
    } else {
        SetupCheckItem {
            key: "rbac".to_string(),
            title: "RBAC Guard".to_string(),
            status: SetupCheckStatus::Warn,
            critical: true,
            message: "RBAC guard is disabled; protected routes run without permission checks."
                .to_string(),
            remediation: "Set AUTH_RBAC_ENABLED=true before production rollout.".to_string(),
        }
    }
}

fn check_oidc_settings(state: &AppState) -> SetupCheckItem {
    if !state.oidc.enabled {
        return SetupCheckItem {
            key: "oidc".to_string(),
            title: "OIDC Integration".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message: "OIDC is disabled; platform is running in header/local auth mode.".to_string(),
            remediation:
                "Enable AUTH_OIDC_ENABLED and configure OIDC endpoints when integrating enterprise SSO."
                    .to_string(),
        };
    }

    if state.oidc.redirect_uri.is_none() {
        return SetupCheckItem {
            key: "oidc".to_string(),
            title: "OIDC Integration".to_string(),
            status: SetupCheckStatus::Fail,
            critical: true,
            message: "OIDC is enabled but AUTH_OIDC_REDIRECT_URI is missing.".to_string(),
            remediation: "Set AUTH_OIDC_REDIRECT_URI to the API callback URL and retry."
                .to_string(),
        };
    }

    if !state.oidc.dev_mode_enabled
        && (state.oidc.authorization_endpoint.is_none()
            || state.oidc.token_endpoint.is_none()
            || state.oidc.userinfo_endpoint.is_none()
            || state.oidc.client_id.is_none()
            || state.oidc.client_secret.is_none())
    {
        return SetupCheckItem {
            key: "oidc".to_string(),
            title: "OIDC Integration".to_string(),
            status: SetupCheckStatus::Fail,
            critical: true,
            message:
                "OIDC production fields are incomplete (authorization/token/userinfo/client credentials)."
                    .to_string(),
            remediation:
                "Configure AUTH_OIDC_AUTHORIZATION_ENDPOINT, AUTH_OIDC_TOKEN_ENDPOINT, AUTH_OIDC_USERINFO_ENDPOINT, AUTH_OIDC_CLIENT_ID, and AUTH_OIDC_CLIENT_SECRET."
                    .to_string(),
        };
    }

    SetupCheckItem {
        key: "oidc".to_string(),
        title: "OIDC Integration".to_string(),
        status: SetupCheckStatus::Pass,
        critical: true,
        message: "OIDC settings are complete for the current mode.".to_string(),
        remediation: "If login callbacks fail, verify IdP redirect URI and token/userinfo endpoint reachability.".to_string(),
    }
}

fn check_monitoring_secret_settings(state: &AppState) -> SetupCheckItem {
    if state.monitoring_secret.inline_policy == "allow"
        && state.monitoring_secret.encryption_key.is_none()
    {
        return SetupCheckItem {
            key: "monitoring-secret-key".to_string(),
            title: "Monitoring Secret Encryption".to_string(),
            status: SetupCheckStatus::Fail,
            critical: true,
            message: "Inline monitoring secret policy is allow, but MONITORING_SECRET_ENCRYPTION_KEY is missing.".to_string(),
            remediation: "Set MONITORING_SECRET_ENCRYPTION_KEY (base64-encoded 32-byte key) or switch MONITORING_SECRET_INLINE_POLICY=forbid and use env:SECRET refs.".to_string(),
        };
    }

    if state.monitoring_secret.inline_policy == "forbid" {
        return SetupCheckItem {
            key: "monitoring-secret-key".to_string(),
            title: "Monitoring Secret Encryption".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: "Inline monitoring secrets are disabled; env-ref mode is enforced."
                .to_string(),
            remediation:
                "Use secret_ref values in env:KEY format for monitoring source credentials."
                    .to_string(),
        };
    }

    SetupCheckItem {
        key: "monitoring-secret-key".to_string(),
        title: "Monitoring Secret Encryption".to_string(),
        status: SetupCheckStatus::Pass,
        critical: true,
        message: "Monitoring secret encryption key is configured.".to_string(),
        remediation: "Rotate encryption key through controlled migration procedures only."
            .to_string(),
    }
}

fn check_workflow_policy_mode(state: &AppState) -> SetupCheckItem {
    match state.workflow_execution.policy_mode.as_str() {
        "disabled" => SetupCheckItem {
            key: "workflow-policy".to_string(),
            title: "Workflow Execution Policy".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message: "Workflow execution policy is disabled; automation steps will not run scripts.".to_string(),
            remediation: "Set WORKFLOW_EXECUTION_POLICY_MODE to allowlist or sandboxed when enabling automation playbooks.".to_string(),
        },
        "allowlist" => SetupCheckItem {
            key: "workflow-policy".to_string(),
            title: "Workflow Execution Policy".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: "Workflow execution is enabled in allowlist mode.".to_string(),
            remediation: "Keep WORKFLOW_EXECUTION_ALLOWLIST curated and audited.".to_string(),
        },
        "sandboxed" => SetupCheckItem {
            key: "workflow-policy".to_string(),
            title: "Workflow Execution Policy".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: "Workflow execution is enabled in sandboxed mode.".to_string(),
            remediation: "Verify sandbox directory permissions and execution boundaries regularly.".to_string(),
        },
        other => SetupCheckItem {
            key: "workflow-policy".to_string(),
            title: "Workflow Execution Policy".to_string(),
            status: SetupCheckStatus::Fail,
            critical: true,
            message: format!("Unsupported workflow execution mode: {other}"),
            remediation:
                "Use one of the supported modes: disabled, allowlist, sandboxed.".to_string(),
        },
    }
}

async fn check_monitoring_source_seed(db: sqlx::PgPool) -> SetupCheckItem {
    match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM monitoring_sources WHERE is_enabled = TRUE",
    )
    .fetch_one(&db)
    .await
    {
        Ok(total) if total > 0 => SetupCheckItem {
            key: "monitoring-source-seed".to_string(),
            title: "Monitoring Source Bootstrap".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: format!("{total} enabled monitoring source(s) are configured."),
            remediation: "Add additional sources by site/department to improve coverage.".to_string(),
        },
        Ok(_) => SetupCheckItem {
            key: "monitoring-source-seed".to_string(),
            title: "Monitoring Source Bootstrap".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message: "No enabled monitoring source found.".to_string(),
            remediation:
                "Create at least one monitoring source in the console before using monitoring and alert features."
                    .to_string(),
        },
        Err(err) => SetupCheckItem {
            key: "monitoring-source-seed".to_string(),
            title: "Monitoring Source Bootstrap".to_string(),
            status: SetupCheckStatus::Fail,
            critical: false,
            message: format!("Unable to verify monitoring source count: {err}"),
            remediation:
                "Check database migration status for monitoring tables and rerun setup checks."
                    .to_string(),
        },
    }
}

async fn check_alert_policy_templates(db: sqlx::PgPool) -> SetupCheckItem {
    match sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM alert_ticket_policies")
        .fetch_one(&db)
        .await
    {
        Ok(total) if total >= 3 => SetupCheckItem {
            key: "alert-policy-templates".to_string(),
            title: "Alert Ticket Policy Templates".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: format!("{total} policy template(s) are available."),
            remediation: "Review policy enablement and dedup windows before production rollout."
                .to_string(),
        },
        Ok(total) => SetupCheckItem {
            key: "alert-policy-templates".to_string(),
            title: "Alert Ticket Policy Templates".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message: format!(
                "Only {total} policy template(s) detected; expected at least 3 defaults."
            ),
            remediation: "Run latest migrations and verify default alert ticket policy seeds."
                .to_string(),
        },
        Err(err) => SetupCheckItem {
            key: "alert-policy-templates".to_string(),
            title: "Alert Ticket Policy Templates".to_string(),
            status: SetupCheckStatus::Fail,
            critical: false,
            message: format!("Unable to query alert policy templates: {err}"),
            remediation: "Ensure alerting migrations are applied successfully before onboarding."
                .to_string(),
        },
    }
}

async fn tcp_endpoint_check(
    key: &str,
    title: &str,
    addr: &str,
    critical: bool,
    remediation: &str,
) -> SetupCheckItem {
    let timeout_window = Duration::from_millis(TCP_CHECK_TIMEOUT_MS);
    let check = timeout(timeout_window, TcpStream::connect(addr)).await;

    match check {
        Ok(Ok(_)) => SetupCheckItem {
            key: key.to_string(),
            title: title.to_string(),
            status: SetupCheckStatus::Pass,
            critical,
            message: format!("TCP reachability check passed: {addr}"),
            remediation: remediation.to_string(),
        },
        Ok(Err(err)) => SetupCheckItem {
            key: key.to_string(),
            title: title.to_string(),
            status: SetupCheckStatus::Fail,
            critical,
            message: format!("TCP reachability check failed for {addr}: {err}"),
            remediation: remediation.to_string(),
        },
        Err(_) => SetupCheckItem {
            key: key.to_string(),
            title: title.to_string(),
            status: SetupCheckStatus::Fail,
            critical,
            message: format!("TCP reachability check timed out for {addr}."),
            remediation: remediation.to_string(),
        },
    }
}

fn read_addr(env_key: &str, default_value: &str) -> String {
    env::var(env_key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

trait SetupItemKey {
    fn with_key(self, key: &str) -> Self;
}

impl SetupItemKey for SetupCheckItem {
    fn with_key(mut self, key: &str) -> Self {
        self.key = key.to_string();
        self
    }
}
