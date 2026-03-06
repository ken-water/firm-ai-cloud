use std::{collections::BTreeSet, env, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::FromRow;
use tokio::{net::TcpStream, time::timeout};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const DEFAULT_WEB_ADDR: &str = "127.0.0.1:8081";
const DEFAULT_API_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_REDIS_ADDR: &str = "127.0.0.1:6379";
const DEFAULT_OPENSEARCH_ADDR: &str = "127.0.0.1:9200";
const DEFAULT_MINIO_ADDR: &str = "127.0.0.1:9000";
const DEFAULT_ZABBIX_ADDR: &str = "127.0.0.1:10051";
const TCP_CHECK_TIMEOUT_MS: u64 = 900;
const SETUP_TEMPLATE_IDENTITY: &str = "identity-safe-baseline";
const SETUP_TEMPLATE_MONITORING: &str = "monitoring-zabbix-bootstrap";
const SETUP_TEMPLATE_NOTIFICATION: &str = "notification-oncall-bootstrap";
const MAX_TEMPLATE_TEXT_LEN: usize = 512;
const MAX_TEMPLATE_USERS: usize = 32;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/preflight", get(get_setup_preflight))
        .route("/checklist", get(get_setup_checklist))
        .route("/templates", get(list_setup_templates))
        .route("/templates/{key}/preview", post(preview_setup_template))
        .route("/templates/{key}/apply", post(apply_setup_template))
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

#[derive(Debug, Serialize)]
struct SetupTemplateCatalogResponse {
    items: Vec<SetupTemplateCatalogItem>,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SetupTemplateCatalogItem {
    key: String,
    name: String,
    category: String,
    description: Option<String>,
    param_schema: Value,
    apply_plan: Value,
    rollback_hints: Vec<String>,
    is_enabled: bool,
    is_system: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SetupTemplateValidationError {
    field: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct SetupTemplatePreviewAction {
    action_key: String,
    summary: String,
    outcome: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct SetupTemplatePreviewResponse {
    template: SetupTemplateCatalogItem,
    ready: bool,
    validation_errors: Vec<SetupTemplateValidationError>,
    actions: Vec<SetupTemplatePreviewAction>,
    rollback_hints: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SetupTemplateApplyAction {
    action_key: String,
    outcome: String,
    target_id: Option<String>,
    detail: String,
}

#[derive(Debug, Serialize)]
struct SetupTemplateApplyResponse {
    actor: String,
    template_key: String,
    status: String,
    applied_actions: Vec<SetupTemplateApplyAction>,
    rollback_hints: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SetupTemplateRequest {
    params: Option<Value>,
    note: Option<String>,
}

#[derive(Debug, FromRow)]
struct SetupTemplateRow {
    id: i64,
    template_key: String,
    name: String,
    category: String,
    description: Option<String>,
    param_schema: Value,
    apply_plan: Value,
    rollback_hints: Value,
    is_enabled: bool,
    is_system: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug)]
struct IdentityTemplateParams {
    identity_mode: String,
    break_glass_users: Vec<String>,
}

#[derive(Debug)]
struct MonitoringTemplateParams {
    name: String,
    endpoint: String,
    auth_type: String,
    username: Option<String>,
    secret_ref: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug)]
struct NotificationTemplateParams {
    channel_name: String,
    channel_type: String,
    target: String,
    event_type: String,
    site: Option<String>,
    department: Option<String>,
    title_template: String,
    body_template: String,
}

#[derive(Debug)]
enum SetupTemplateInput {
    Identity(IdentityTemplateParams),
    Monitoring(MonitoringTemplateParams),
    Notification(NotificationTemplateParams),
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
    checks.push(check_setup_template_baseline(state.db.clone()).await);
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

async fn list_setup_templates(
    State(state): State<AppState>,
) -> AppResult<Json<SetupTemplateCatalogResponse>> {
    let rows: Vec<SetupTemplateRow> = sqlx::query_as(
        "SELECT
            id,
            template_key,
            name,
            category,
            description,
            param_schema,
            apply_plan,
            rollback_hints,
            is_enabled,
            is_system,
            updated_at
         FROM setup_bootstrap_templates
         WHERE is_enabled = TRUE
         ORDER BY category ASC, template_key ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .iter()
        .map(SetupTemplateCatalogItem::from)
        .collect::<Vec<_>>();
    Ok(Json(SetupTemplateCatalogResponse {
        total: items.len(),
        items,
    }))
}

async fn preview_setup_template(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<SetupTemplateRequest>,
) -> AppResult<Json<SetupTemplatePreviewResponse>> {
    let template = load_setup_template_by_key(&state, key.as_str()).await?;
    let template_item = SetupTemplateCatalogItem::from(&template);
    let params = normalize_template_params(payload.params)?;

    let input = match validate_setup_template_input(template.template_key.as_str(), &params) {
        Ok(value) => value,
        Err(validation_errors) => {
            return Ok(Json(SetupTemplatePreviewResponse {
                template: template_item.clone(),
                ready: false,
                validation_errors,
                actions: Vec::new(),
                rollback_hints: template_item.rollback_hints,
            }));
        }
    };

    let actions = preview_setup_template_actions(&state, &input).await?;
    Ok(Json(SetupTemplatePreviewResponse {
        template: template_item.clone(),
        ready: true,
        validation_errors: Vec::new(),
        actions,
        rollback_hints: template_item.rollback_hints,
    }))
}

async fn apply_setup_template(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<SetupTemplateRequest>,
) -> AppResult<Json<SetupTemplateApplyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template = load_setup_template_by_key(&state, key.as_str()).await?;
    let params = normalize_template_params(payload.params)?;
    let input = validate_setup_template_input(template.template_key.as_str(), &params)
        .map_err(first_template_validation_error)?;
    let note = trim_optional(payload.note, MAX_TEMPLATE_TEXT_LEN);

    let applied_actions = apply_setup_template_input(&state, actor.as_str(), &input).await?;

    sqlx::query(
        "INSERT INTO setup_bootstrap_template_runs
            (template_id, template_key, actor, status, params, result, error_message)
         VALUES ($1, $2, $3, 'applied', $4, $5, NULL)",
    )
    .bind(template.id)
    .bind(template.template_key.as_str())
    .bind(actor.as_str())
    .bind(Value::Object(params.clone()))
    .bind(json!({
        "actions": applied_actions,
        "note": note
    }))
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "setup.template.apply".to_string(),
            target_type: "setup_bootstrap_template".to_string(),
            target_id: Some(template.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.template_key,
                "actions": applied_actions
            }),
        },
    )
    .await;

    Ok(Json(SetupTemplateApplyResponse {
        actor,
        template_key: template.template_key,
        status: "applied".to_string(),
        applied_actions,
        rollback_hints: parse_rollback_hints(&template.rollback_hints),
    }))
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

impl From<&SetupTemplateRow> for SetupTemplateCatalogItem {
    fn from(row: &SetupTemplateRow) -> Self {
        Self {
            key: row.template_key.clone(),
            name: row.name.clone(),
            category: row.category.clone(),
            description: row.description.clone(),
            param_schema: row.param_schema.clone(),
            apply_plan: row.apply_plan.clone(),
            rollback_hints: parse_rollback_hints(&row.rollback_hints),
            is_enabled: row.is_enabled,
            is_system: row.is_system,
            updated_at: row.updated_at,
        }
    }
}

fn parse_rollback_hints(raw: &Value) -> Vec<String> {
    let Value::Array(items) = raw else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

async fn load_setup_template_by_key(state: &AppState, key: &str) -> AppResult<SetupTemplateRow> {
    let normalized = key.trim().to_ascii_lowercase();
    let row: Option<SetupTemplateRow> = sqlx::query_as(
        "SELECT
            id,
            template_key,
            name,
            category,
            description,
            param_schema,
            apply_plan,
            rollback_hints,
            is_enabled,
            is_system,
            updated_at
         FROM setup_bootstrap_templates
         WHERE template_key = $1
           AND is_enabled = TRUE
         LIMIT 1",
    )
    .bind(normalized)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| AppError::Validation("setup template is not found or disabled".to_string()))
}

fn normalize_template_params(params: Option<Value>) -> AppResult<JsonMap<String, Value>> {
    let value = params.unwrap_or_else(|| Value::Object(JsonMap::new()));
    let Value::Object(object) = value else {
        return Err(AppError::Validation(
            "params must be a JSON object".to_string(),
        ));
    };
    Ok(object)
}

fn validate_setup_template_input(
    template_key: &str,
    params: &JsonMap<String, Value>,
) -> Result<SetupTemplateInput, Vec<SetupTemplateValidationError>> {
    match template_key {
        SETUP_TEMPLATE_IDENTITY => {
            validate_identity_template_params(params).map(SetupTemplateInput::Identity)
        }
        SETUP_TEMPLATE_MONITORING => {
            validate_monitoring_template_params(params).map(SetupTemplateInput::Monitoring)
        }
        SETUP_TEMPLATE_NOTIFICATION => {
            validate_notification_template_params(params).map(SetupTemplateInput::Notification)
        }
        _ => Err(vec![SetupTemplateValidationError {
            field: "template".to_string(),
            message: format!("unsupported setup template key '{template_key}'"),
        }]),
    }
}

fn first_template_validation_error(errors: Vec<SetupTemplateValidationError>) -> AppError {
    let message = errors
        .into_iter()
        .next()
        .map(|item| format!("{}: {}", item.field, item.message))
        .unwrap_or_else(|| "invalid template params".to_string());
    AppError::Validation(message)
}

fn validate_identity_template_params(
    params: &JsonMap<String, Value>,
) -> Result<IdentityTemplateParams, Vec<SetupTemplateValidationError>> {
    let mut errors = Vec::new();
    let identity_mode = match string_param(params, "identity_mode", true, 64) {
        Ok(Some(value)) => {
            let normalized = value.to_ascii_lowercase();
            match normalized.as_str() {
                "break_glass_only" | "disabled" | "allow_all" => normalized,
                _ => {
                    errors.push(SetupTemplateValidationError {
                        field: "identity_mode".to_string(),
                        message: "must be one of: break_glass_only, disabled, allow_all"
                            .to_string(),
                    });
                    String::new()
                }
            }
        }
        Ok(None) => String::new(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "identity_mode".to_string(),
                message,
            });
            String::new()
        }
    };

    let break_glass_raw = match string_param(params, "break_glass_users", false, 1024) {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "break_glass_users".to_string(),
                message,
            });
            String::new()
        }
    };

    let break_glass_users = break_glass_raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_TEMPLATE_USERS)
        .collect::<Vec<_>>();

    if identity_mode == "break_glass_only" && break_glass_users.is_empty() {
        errors.push(SetupTemplateValidationError {
            field: "break_glass_users".to_string(),
            message: "is required when identity_mode=break_glass_only".to_string(),
        });
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(IdentityTemplateParams {
        identity_mode,
        break_glass_users,
    })
}

fn validate_monitoring_template_params(
    params: &JsonMap<String, Value>,
) -> Result<MonitoringTemplateParams, Vec<SetupTemplateValidationError>> {
    let mut errors = Vec::new();
    let name = collect_required_param(params, "name", 128, &mut errors);
    let endpoint = collect_required_param(params, "endpoint", 512, &mut errors);
    let secret_ref = collect_required_param(params, "secret_ref", 255, &mut errors);

    let auth_type = match string_param(params, "auth_type", false, 32) {
        Ok(Some(value)) => {
            let normalized = value.to_ascii_lowercase();
            if normalized == "token" || normalized == "basic" {
                normalized
            } else {
                errors.push(SetupTemplateValidationError {
                    field: "auth_type".to_string(),
                    message: "must be token or basic".to_string(),
                });
                "token".to_string()
            }
        }
        Ok(None) => "token".to_string(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "auth_type".to_string(),
                message,
            });
            "token".to_string()
        }
    };

    let username = match string_param(params, "username", false, 128) {
        Ok(value) => value,
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "username".to_string(),
                message,
            });
            None
        }
    };
    let site = match string_param(params, "site", false, 64) {
        Ok(value) => value,
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "site".to_string(),
                message,
            });
            None
        }
    };
    let department = match string_param(params, "department", false, 64) {
        Ok(value) => value,
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "department".to_string(),
                message,
            });
            None
        }
    };

    if auth_type == "basic" && username.is_none() {
        errors.push(SetupTemplateValidationError {
            field: "username".to_string(),
            message: "is required when auth_type=basic".to_string(),
        });
    }
    if !secret_ref.starts_with("env:") {
        errors.push(SetupTemplateValidationError {
            field: "secret_ref".to_string(),
            message: "must use env:KEY format (example: env:ZABBIX_API_TOKEN)".to_string(),
        });
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(MonitoringTemplateParams {
        name,
        endpoint,
        auth_type,
        username,
        secret_ref,
        site,
        department,
    })
}

fn validate_notification_template_params(
    params: &JsonMap<String, Value>,
) -> Result<NotificationTemplateParams, Vec<SetupTemplateValidationError>> {
    let mut errors = Vec::new();
    let channel_name = collect_required_param(params, "channel_name", 128, &mut errors);
    let target = collect_required_param(params, "target", 512, &mut errors);
    let event_type = collect_required_param(params, "event_type", 64, &mut errors);

    let channel_type = match string_param(params, "channel_type", false, 32) {
        Ok(Some(value)) => {
            let normalized = value.to_ascii_lowercase();
            if normalized == "email" || normalized == "webhook" {
                normalized
            } else {
                errors.push(SetupTemplateValidationError {
                    field: "channel_type".to_string(),
                    message: "must be email or webhook".to_string(),
                });
                "webhook".to_string()
            }
        }
        Ok(None) => "webhook".to_string(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "channel_type".to_string(),
                message,
            });
            "webhook".to_string()
        }
    };

    let title_template = match string_param(params, "title_template", false, MAX_TEMPLATE_TEXT_LEN)
    {
        Ok(Some(value)) => value,
        Ok(None) => "Discovery Event: {{event_type}}".to_string(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "title_template".to_string(),
                message,
            });
            String::new()
        }
    };
    let body_template = match string_param(params, "body_template", false, MAX_TEMPLATE_TEXT_LEN) {
        Ok(Some(value)) => value,
        Ok(None) => "{{payload}}".to_string(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "body_template".to_string(),
                message,
            });
            String::new()
        }
    };
    let site = match string_param(params, "site", false, 128) {
        Ok(value) => value,
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "site".to_string(),
                message,
            });
            None
        }
    };
    let department = match string_param(params, "department", false, 128) {
        Ok(value) => value,
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: "department".to_string(),
                message,
            });
            None
        }
    };

    if !event_type
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_'))
    {
        errors.push(SetupTemplateValidationError {
            field: "event_type".to_string(),
            message: "contains unsupported characters".to_string(),
        });
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(NotificationTemplateParams {
        channel_name,
        channel_type,
        target,
        event_type,
        site,
        department,
        title_template,
        body_template,
    })
}

fn collect_required_param(
    params: &JsonMap<String, Value>,
    key: &str,
    max_len: usize,
    errors: &mut Vec<SetupTemplateValidationError>,
) -> String {
    match string_param(params, key, true, max_len) {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(message) => {
            errors.push(SetupTemplateValidationError {
                field: key.to_string(),
                message,
            });
            String::new()
        }
    }
}

fn string_param(
    params: &JsonMap<String, Value>,
    key: &str,
    required: bool,
    max_len: usize,
) -> Result<Option<String>, String> {
    let Some(raw) = params.get(key) else {
        if required {
            return Err("is required".to_string());
        }
        return Ok(None);
    };

    let Some(raw_str) = raw.as_str() else {
        return Err("must be a string".to_string());
    };
    let trimmed = raw_str.trim();
    if trimmed.is_empty() {
        if required {
            return Err("cannot be empty".to_string());
        }
        return Ok(None);
    }
    if trimmed.len() > max_len {
        return Err(format!("length must be <= {max_len}"));
    }
    Ok(Some(trimmed.to_string()))
}

async fn preview_setup_template_actions(
    state: &AppState,
    input: &SetupTemplateInput,
) -> AppResult<Vec<SetupTemplatePreviewAction>> {
    match input {
        SetupTemplateInput::Identity(_) => {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM setup_identity_preferences)")
                    .fetch_one(&state.db)
                    .await?;
            Ok(vec![SetupTemplatePreviewAction {
                action_key: "identity.preference".to_string(),
                summary: "Persist identity-mode baseline preference".to_string(),
                outcome: if exists {
                    "update".to_string()
                } else {
                    "create".to_string()
                },
                detail: "Stores desired fallback governance profile for rollout tracking."
                    .to_string(),
            }])
        }
        SetupTemplateInput::Monitoring(params) => {
            let existing_id: Option<i64> = sqlx::query_scalar(
                "SELECT id
                 FROM monitoring_sources
                 WHERE name = $1
                 LIMIT 1",
            )
            .bind(params.name.as_str())
            .fetch_optional(&state.db)
            .await?;
            Ok(vec![SetupTemplatePreviewAction {
                action_key: "monitoring.source".to_string(),
                summary: "Create or update monitoring source".to_string(),
                outcome: if existing_id.is_some() {
                    "update".to_string()
                } else {
                    "create".to_string()
                },
                detail: format!(
                    "Source '{}' will point to endpoint '{}' with auth '{}'.",
                    params.name, params.endpoint, params.auth_type
                ),
            }])
        }
        SetupTemplateInput::Notification(params) => {
            let channel_id: Option<i64> = sqlx::query_scalar(
                "SELECT id
                 FROM discovery_notification_channels
                 WHERE name = $1
                   AND channel_type = $2
                   AND target = $3
                 LIMIT 1",
            )
            .bind(params.channel_name.as_str())
            .bind(params.channel_type.as_str())
            .bind(params.target.as_str())
            .fetch_optional(&state.db)
            .await?;
            let template_id: Option<i64> = sqlx::query_scalar(
                "SELECT id
                 FROM discovery_notification_templates
                 WHERE event_type = $1
                 ORDER BY id ASC
                 LIMIT 1",
            )
            .bind(params.event_type.as_str())
            .fetch_optional(&state.db)
            .await?;
            let subscription_exists = if let Some(channel_id) = channel_id {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(
                        SELECT 1
                        FROM discovery_notification_subscriptions
                        WHERE channel_id = $1
                          AND event_type = $2
                          AND coalesce(site, '') = coalesce($3, '')
                          AND coalesce(department, '') = coalesce($4, '')
                    )",
                )
                .bind(channel_id)
                .bind(params.event_type.as_str())
                .bind(params.site.as_deref())
                .bind(params.department.as_deref())
                .fetch_one(&state.db)
                .await?
            } else {
                false
            };
            Ok(vec![
                SetupTemplatePreviewAction {
                    action_key: "notification.channel".to_string(),
                    summary: "Create or update notification channel".to_string(),
                    outcome: if channel_id.is_some() {
                        "update".to_string()
                    } else {
                        "create".to_string()
                    },
                    detail: format!(
                        "Channel '{}' ({}) -> {}",
                        params.channel_name, params.channel_type, params.target
                    ),
                },
                SetupTemplatePreviewAction {
                    action_key: "notification.template".to_string(),
                    summary: "Create or update event template".to_string(),
                    outcome: if template_id.is_some() {
                        "update".to_string()
                    } else {
                        "create".to_string()
                    },
                    detail: format!(
                        "Template event_type='{}' with title/body placeholders",
                        params.event_type
                    ),
                },
                SetupTemplatePreviewAction {
                    action_key: "notification.subscription".to_string(),
                    summary: "Create or update channel subscription".to_string(),
                    outcome: if subscription_exists {
                        "noop".to_string()
                    } else {
                        "create".to_string()
                    },
                    detail: format!(
                        "Subscription scope site='{}', department='{}'",
                        params.site.as_deref().unwrap_or("*"),
                        params.department.as_deref().unwrap_or("*")
                    ),
                },
            ])
        }
    }
}

async fn apply_setup_template_input(
    state: &AppState,
    actor: &str,
    input: &SetupTemplateInput,
) -> AppResult<Vec<SetupTemplateApplyAction>> {
    let mut tx = state.db.begin().await?;
    let actions = match input {
        SetupTemplateInput::Identity(params) => {
            sqlx::query(
                "INSERT INTO setup_identity_preferences
                    (id, identity_mode, break_glass_users, updated_by, updated_at)
                 VALUES (1, $1, $2, $3, NOW())
                 ON CONFLICT (id)
                 DO UPDATE SET
                    identity_mode = EXCLUDED.identity_mode,
                    break_glass_users = EXCLUDED.break_glass_users,
                    updated_by = EXCLUDED.updated_by,
                    updated_at = NOW()",
            )
            .bind(params.identity_mode.as_str())
            .bind(json!(params.break_glass_users))
            .bind(actor)
            .execute(&mut *tx)
            .await?;

            vec![SetupTemplateApplyAction {
                action_key: "identity.preference".to_string(),
                outcome: "applied".to_string(),
                target_id: Some("1".to_string()),
                detail: format!(
                    "identity_mode='{}', break_glass_users={}",
                    params.identity_mode,
                    params.break_glass_users.join(",")
                ),
            }]
        }
        SetupTemplateInput::Monitoring(params) => {
            let existing_id: Option<i64> = sqlx::query_scalar(
                "SELECT id
                 FROM monitoring_sources
                 WHERE name = $1
                 LIMIT 1",
            )
            .bind(params.name.as_str())
            .fetch_optional(&mut *tx)
            .await?;

            let source_id = if let Some(source_id) = existing_id {
                sqlx::query(
                    "UPDATE monitoring_sources
                     SET source_type = 'zabbix',
                         endpoint = $2,
                         proxy_endpoint = NULL,
                         auth_type = $3,
                         username = $4,
                         secret_ref = $5,
                         secret_ciphertext = NULL,
                         site = $6,
                         department = $7,
                         is_enabled = TRUE,
                         updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(source_id)
                .bind(params.endpoint.as_str())
                .bind(params.auth_type.as_str())
                .bind(params.username.as_deref())
                .bind(params.secret_ref.as_str())
                .bind(params.site.as_deref())
                .bind(params.department.as_deref())
                .execute(&mut *tx)
                .await?;
                source_id
            } else {
                sqlx::query_scalar(
                    "INSERT INTO monitoring_sources
                        (name, source_type, endpoint, proxy_endpoint, auth_type, username, secret_ref, secret_ciphertext, site, department, is_enabled)
                     VALUES ($1, 'zabbix', $2, NULL, $3, $4, $5, NULL, $6, $7, TRUE)
                     RETURNING id",
                )
                .bind(params.name.as_str())
                .bind(params.endpoint.as_str())
                .bind(params.auth_type.as_str())
                .bind(params.username.as_deref())
                .bind(params.secret_ref.as_str())
                .bind(params.site.as_deref())
                .bind(params.department.as_deref())
                .fetch_one(&mut *tx)
                .await?
            };

            vec![SetupTemplateApplyAction {
                action_key: "monitoring.source".to_string(),
                outcome: "applied".to_string(),
                target_id: Some(source_id.to_string()),
                detail: format!("source='{}', endpoint='{}'", params.name, params.endpoint),
            }]
        }
        SetupTemplateInput::Notification(params) => {
            let channel_id: i64 = if let Some(existing_id) = sqlx::query_scalar(
                "SELECT id
                 FROM discovery_notification_channels
                 WHERE name = $1
                   AND channel_type = $2
                   AND target = $3
                 LIMIT 1",
            )
            .bind(params.channel_name.as_str())
            .bind(params.channel_type.as_str())
            .bind(params.target.as_str())
            .fetch_optional(&mut *tx)
            .await?
            {
                sqlx::query(
                    "UPDATE discovery_notification_channels
                     SET config = '{}'::jsonb,
                         is_enabled = TRUE,
                         updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(existing_id)
                .execute(&mut *tx)
                .await?;
                existing_id
            } else {
                sqlx::query_scalar(
                    "INSERT INTO discovery_notification_channels
                        (name, channel_type, target, config, is_enabled)
                     VALUES ($1, $2, $3, '{}'::jsonb, TRUE)
                     RETURNING id",
                )
                .bind(params.channel_name.as_str())
                .bind(params.channel_type.as_str())
                .bind(params.target.as_str())
                .fetch_one(&mut *tx)
                .await?
            };

            let template_id: i64 = if let Some(existing_id) = sqlx::query_scalar(
                "SELECT id
                 FROM discovery_notification_templates
                 WHERE event_type = $1
                 ORDER BY id ASC
                 LIMIT 1",
            )
            .bind(params.event_type.as_str())
            .fetch_optional(&mut *tx)
            .await?
            {
                sqlx::query(
                    "UPDATE discovery_notification_templates
                     SET title_template = $2,
                         body_template = $3,
                         is_enabled = TRUE,
                         updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(existing_id)
                .bind(params.title_template.as_str())
                .bind(params.body_template.as_str())
                .execute(&mut *tx)
                .await?;
                existing_id
            } else {
                sqlx::query_scalar(
                    "INSERT INTO discovery_notification_templates
                        (event_type, title_template, body_template, is_enabled)
                     VALUES ($1, $2, $3, TRUE)
                     RETURNING id",
                )
                .bind(params.event_type.as_str())
                .bind(params.title_template.as_str())
                .bind(params.body_template.as_str())
                .fetch_one(&mut *tx)
                .await?
            };

            let subscription_id: i64 = if let Some(existing_id) = sqlx::query_scalar(
                "SELECT id
                 FROM discovery_notification_subscriptions
                 WHERE channel_id = $1
                   AND event_type = $2
                   AND coalesce(site, '') = coalesce($3, '')
                   AND coalesce(department, '') = coalesce($4, '')
                 LIMIT 1",
            )
            .bind(channel_id)
            .bind(params.event_type.as_str())
            .bind(params.site.as_deref())
            .bind(params.department.as_deref())
            .fetch_optional(&mut *tx)
            .await?
            {
                sqlx::query(
                    "UPDATE discovery_notification_subscriptions
                     SET is_enabled = TRUE,
                         updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(existing_id)
                .execute(&mut *tx)
                .await?;
                existing_id
            } else {
                sqlx::query_scalar(
                    "INSERT INTO discovery_notification_subscriptions
                        (channel_id, event_type, site, department, is_enabled)
                     VALUES ($1, $2, $3, $4, TRUE)
                     RETURNING id",
                )
                .bind(channel_id)
                .bind(params.event_type.as_str())
                .bind(params.site.as_deref())
                .bind(params.department.as_deref())
                .fetch_one(&mut *tx)
                .await?
            };

            vec![
                SetupTemplateApplyAction {
                    action_key: "notification.channel".to_string(),
                    outcome: "applied".to_string(),
                    target_id: Some(channel_id.to_string()),
                    detail: format!(
                        "channel='{}' ({})",
                        params.channel_name, params.channel_type
                    ),
                },
                SetupTemplateApplyAction {
                    action_key: "notification.template".to_string(),
                    outcome: "applied".to_string(),
                    target_id: Some(template_id.to_string()),
                    detail: format!("event_type='{}'", params.event_type),
                },
                SetupTemplateApplyAction {
                    action_key: "notification.subscription".to_string(),
                    outcome: "applied".to_string(),
                    target_id: Some(subscription_id.to_string()),
                    detail: format!(
                        "scope site='{}', department='{}'",
                        params.site.as_deref().unwrap_or("*"),
                        params.department.as_deref().unwrap_or("*")
                    ),
                },
            ]
        }
    };

    tx.commit().await?;
    Ok(actions)
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            let capped = if trimmed.len() > max_len {
                trimmed[..max_len].to_string()
            } else {
                trimmed.to_string()
            };
            Some(capped)
        }
    })
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

async fn check_setup_template_baseline(db: sqlx::PgPool) -> SetupCheckItem {
    let enabled_templates = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM setup_bootstrap_templates WHERE is_enabled = TRUE",
    )
    .fetch_one(&db)
    .await;
    let applied_runs = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM setup_bootstrap_template_runs WHERE status = 'applied'",
    )
    .fetch_one(&db)
    .await;

    match (enabled_templates, applied_runs) {
        (Ok(template_total), Ok(run_total)) if template_total > 0 && run_total > 0 => SetupCheckItem {
            key: "setup-template-baseline".to_string(),
            title: "Setup Template Baseline".to_string(),
            status: SetupCheckStatus::Pass,
            critical: false,
            message: format!(
                "{template_total} enabled template(s), {run_total} applied template run(s)."
            ),
            remediation:
                "Review latest template runs and keep identity/monitoring/notification settings aligned with production policy."
                    .to_string(),
        },
        (Ok(template_total), Ok(_)) if template_total > 0 => SetupCheckItem {
            key: "setup-template-baseline".to_string(),
            title: "Setup Template Baseline".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message:
                "No applied setup template run found for current environment.".to_string(),
            remediation:
                "Use /api/v1/setup/templates preview+apply flow to bootstrap identity, monitoring, and notification defaults."
                    .to_string(),
        },
        (Ok(_), Ok(_)) => SetupCheckItem {
            key: "setup-template-baseline".to_string(),
            title: "Setup Template Baseline".to_string(),
            status: SetupCheckStatus::Warn,
            critical: false,
            message: "Setup template catalog has no enabled entries.".to_string(),
            remediation: "Apply latest migrations to restore default setup templates.".to_string(),
        },
        (Err(err), _) | (_, Err(err)) => SetupCheckItem {
            key: "setup-template-baseline".to_string(),
            title: "Setup Template Baseline".to_string(),
            status: SetupCheckStatus::Fail,
            critical: false,
            message: format!("Unable to query setup templates/runs: {err}"),
            remediation:
                "Verify setup template migrations and rerun preflight/checklist endpoints."
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

#[cfg(test)]
mod tests {
    use serde_json::{Map as JsonMap, Value, json};

    use super::{
        parse_rollback_hints, validate_identity_template_params,
        validate_monitoring_template_params, validate_notification_template_params,
    };

    fn json_params(value: Value) -> JsonMap<String, Value> {
        value.as_object().cloned().expect("object")
    }

    #[test]
    fn identity_template_requires_break_glass_users_for_break_glass_mode() {
        let params = json_params(json!({
            "identity_mode": "break_glass_only"
        }));
        let result = validate_identity_template_params(&params);
        assert!(result.is_err());
    }

    #[test]
    fn monitoring_template_requires_env_secret_ref() {
        let params = json_params(json!({
            "name": "zabbix-core",
            "endpoint": "http://127.0.0.1:8082/api_jsonrpc.php",
            "secret_ref": "plain-secret"
        }));
        let result = validate_monitoring_template_params(&params);
        assert!(result.is_err());
    }

    #[test]
    fn notification_template_rejects_invalid_event_type() {
        let params = json_params(json!({
            "channel_name": "oncall-webhook",
            "target": "https://ops.local/hooks",
            "event_type": "bad event"
        }));
        let result = validate_notification_template_params(&params);
        assert!(result.is_err());
    }

    #[test]
    fn rollback_hint_parser_ignores_blank_items() {
        let parsed = parse_rollback_hints(&json!(["step-a", " ", "", "step-b"]));
        assert_eq!(parsed, vec!["step-a".to_string(), "step-b".to_string()]);
    }
}
