use std::{collections::BTreeSet, env, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
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
const SETUP_PROFILE_SMALL_OFFICE: &str = "smb-small-office";
const SETUP_PROFILE_MULTI_SITE_RETAIL: &str = "smb-multi-site-retail";
const SETUP_PROFILE_REGIONAL_ENTERPRISE: &str = "smb-regional-enterprise";
const PROFILE_ALERT_POLICY_KEY: &str = "operator-profile-default-alert";
const PROFILE_BACKUP_POLICY_KEY: &str = "operator-profile-default-backup";
const PROFILE_ESCALATION_POLICY_KEY: &str = "default-ticket-sla";
const MAX_TEMPLATE_TEXT_LEN: usize = 512;
const MAX_TEMPLATE_USERS: usize = 32;
const DEFAULT_PROFILE_HISTORY_LIMIT: u32 = 20;
const MAX_PROFILE_HISTORY_LIMIT: u32 = 100;
const ACTIVATION_SETUP_HREF: &str = "#/setup";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/preflight", get(get_setup_preflight))
        .route("/checklist", get(get_setup_checklist))
        .route("/activation", get(get_setup_activation))
        .route("/templates", get(list_setup_templates))
        .route("/templates/{key}/preview", post(preview_setup_template))
        .route("/templates/{key}/apply", post(apply_setup_template))
        .route("/profiles", get(list_setup_profiles))
        .route("/profiles/history", get(list_setup_profile_history))
        .route(
            "/profiles/history/{id}/revert",
            post(revert_setup_profile_run),
        )
        .route("/profiles/{key}/preview", post(preview_setup_profile))
        .route("/profiles/{key}/apply", post(apply_setup_profile))
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SetupCheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
struct SetupCheckItem {
    key: String,
    title: String,
    status: SetupCheckStatus,
    critical: bool,
    message: String,
    remediation: String,
}

#[derive(Debug, Serialize)]
struct SetupActivationRecommendedAction {
    action_key: String,
    label: String,
    description: String,
    action_type: String,
    href: Option<String>,
    requires_write: bool,
    auto_applicable: bool,
    profile_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct SetupActivationItem {
    item_key: String,
    title: String,
    status: String,
    summary: String,
    reason: String,
    recommended_action: Option<SetupActivationRecommendedAction>,
    evidence: Value,
}

#[derive(Debug, Serialize)]
struct SetupActivationSummary {
    total: usize,
    ready: usize,
    warning: usize,
    blocking: usize,
}

#[derive(Debug, Serialize)]
struct SetupActivationResponse {
    generated_at: DateTime<Utc>,
    overall_status: String,
    recommended_next_step_key: Option<String>,
    recommended_profile_key: Option<String>,
    summary: SetupActivationSummary,
    items: Vec<SetupActivationItem>,
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

#[derive(Debug, Serialize, Clone)]
struct SetupProfileCatalogItem {
    key: String,
    name: String,
    description: String,
    target_scale: String,
    defaults: Value,
}

#[derive(Debug, Serialize)]
struct SetupProfileCatalogResponse {
    items: Vec<SetupProfileCatalogItem>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct SetupProfileChangeSummary {
    domain: String,
    before: String,
    after: String,
    changed: bool,
}

#[derive(Debug, Serialize)]
struct SetupProfilePreviewResponse {
    profile: SetupProfileCatalogItem,
    ready: bool,
    summary: Vec<SetupProfileChangeSummary>,
}

#[derive(Debug, Serialize)]
struct SetupProfileApplyAction {
    action_key: String,
    outcome: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct SetupProfileApplyResponse {
    run_id: i64,
    actor: String,
    profile_key: String,
    status: String,
    actions: Vec<SetupProfileApplyAction>,
    history_hint: String,
}

#[derive(Debug, Serialize, FromRow)]
struct SetupProfileHistoryRecord {
    id: i64,
    profile_key: String,
    profile_name: String,
    actor: String,
    status: String,
    note: Option<String>,
    reverted_by: Option<String>,
    reverted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SetupProfileHistoryResponse {
    items: Vec<SetupProfileHistoryRecord>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct SetupProfileRevertResponse {
    run_id: i64,
    status: String,
    reverted_by: String,
    reverted_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
struct SetupTemplateRequest {
    params: Option<Value>,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SetupProfileRequest {
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SetupProfileHistoryQuery {
    limit: Option<u32>,
    offset: Option<u32>,
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

#[derive(Debug, Clone)]
struct SetupOperatorProfileDefinition {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    target_scale: &'static str,
    identity_mode: &'static str,
    break_glass_users: &'static str,
    alert_dedup_window_seconds: i32,
    alert_ticket_priority: &'static str,
    escalation_near_high_minutes: i32,
    escalation_breach_high_minutes: i32,
    escalation_near_medium_minutes: i32,
    escalation_breach_medium_minutes: i32,
    escalation_owner: &'static str,
    backup_frequency: &'static str,
    backup_schedule_time_utc: &'static str,
    backup_retention_days: i32,
    backup_destination_uri: &'static str,
    drill_frequency: &'static str,
    drill_weekday: Option<i16>,
    drill_time_utc: &'static str,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SetupProfileSnapshot {
    identity: Option<IdentityProfileState>,
    alert_policy: Option<AlertProfileState>,
    escalation_policy: Option<EscalationProfileState>,
    backup_policy: Option<BackupProfileState>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IdentityProfileState {
    identity_mode: String,
    break_glass_users: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AlertProfileState {
    is_enabled: bool,
    dedup_window_seconds: i32,
    ticket_priority: String,
    ticket_category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct EscalationProfileState {
    is_enabled: bool,
    near_high_minutes: i32,
    breach_high_minutes: i32,
    near_medium_minutes: i32,
    breach_medium_minutes: i32,
    escalate_to_assignee: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BackupProfileState {
    frequency: String,
    schedule_time_utc: String,
    schedule_weekday: Option<i16>,
    retention_days: i32,
    destination_type: String,
    destination_uri: String,
    drill_enabled: bool,
    drill_frequency: String,
    drill_weekday: Option<i16>,
    drill_time_utc: String,
}

#[derive(Debug, FromRow)]
struct SetupProfileRunRow {
    id: i64,
    profile_key: String,
    profile_name: String,
    status: String,
    previous_state: Value,
    applied_state: Value,
    reverted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct IdentityProfileStateRow {
    identity_mode: String,
    break_glass_users: Value,
}

#[derive(Debug, FromRow)]
struct AlertProfileStateRow {
    is_enabled: bool,
    dedup_window_seconds: i32,
    ticket_priority: String,
    ticket_category: String,
}

#[derive(Debug, FromRow)]
struct EscalationProfileStateRow {
    is_enabled: bool,
    near_high_minutes: i32,
    breach_high_minutes: i32,
    near_medium_minutes: i32,
    breach_medium_minutes: i32,
    escalate_to_assignee: String,
}

#[derive(Debug, FromRow)]
struct BackupProfileStateRow {
    frequency: String,
    schedule_time_utc: String,
    schedule_weekday: Option<i16>,
    retention_days: i32,
    destination_type: String,
    destination_uri: String,
    drill_enabled: bool,
    drill_frequency: String,
    drill_weekday: Option<i16>,
    drill_time_utc: String,
}

async fn get_setup_preflight(
    State(state): State<AppState>,
) -> AppResult<Json<SetupChecklistResponse>> {
    Ok(Json(build_response(
        "preflight",
        collect_setup_preflight_checks(&state).await,
    )))
}

async fn get_setup_checklist(
    State(state): State<AppState>,
) -> AppResult<Json<SetupChecklistResponse>> {
    Ok(Json(build_response(
        "integration_checklist",
        collect_setup_checklist_checks(&state).await,
    )))
}

async fn get_setup_activation(
    State(state): State<AppState>,
) -> AppResult<Json<SetupActivationResponse>> {
    let preflight = build_response("preflight", collect_setup_preflight_checks(&state).await);
    let checklist = build_response(
        "integration_checklist",
        collect_setup_checklist_checks(&state).await,
    );
    let asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets")
        .fetch_one(&state.db)
        .await?;
    let profile_run_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM setup_operator_profile_runs WHERE status = 'applied'")
            .fetch_one(&state.db)
            .await?;
    let recommended_profile = recommend_setup_profile_key(asset_count);

    let items = vec![
        summarize_activation_checklist_item(
            "environment_preflight",
            "Environment preflight",
            "Review foundational platform readiness before applying operator defaults.",
            &preflight,
            Some(link_activation_action(
                "setup.activation.preflight.review",
                "Review preflight checks",
                "Open the setup wizard and resolve preflight blockers before continuing.",
                ACTIVATION_SETUP_HREF,
                None,
            )),
        ),
        summarize_activation_checklist_item(
            "integration_checklist",
            "Integration checklist",
            "Verify the local stack and baseline integration services are reachable.",
            &checklist,
            Some(link_activation_action(
                "setup.activation.checklist.review",
                "Review integration checklist",
                "Open the setup wizard and resolve service or baseline integration gaps.",
                ACTIVATION_SETUP_HREF,
                None,
            )),
        ),
        build_activation_profile_item(asset_count, profile_run_count, recommended_profile),
    ];

    let summary = summarize_activation_items(&items);
    let overall_status = derive_activation_overall_status(&summary);
    let recommended_next_step_key = select_next_activation_item_key(&items);

    Ok(Json(SetupActivationResponse {
        generated_at: Utc::now(),
        overall_status,
        recommended_next_step_key,
        recommended_profile_key: Some(recommended_profile.to_string()),
        summary,
        items,
    }))
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

async fn list_setup_profiles() -> AppResult<Json<SetupProfileCatalogResponse>> {
    let items = setup_profile_catalog_items();
    Ok(Json(SetupProfileCatalogResponse {
        total: items.len(),
        items,
    }))
}

async fn preview_setup_profile(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(_payload): Json<SetupProfileRequest>,
) -> AppResult<Json<SetupProfilePreviewResponse>> {
    let definition = resolve_setup_profile_definition(key.as_str())?;
    let profile = setup_profile_catalog_item(&definition);
    let current = load_setup_profile_snapshot(&state.db).await?;
    let target = build_setup_profile_snapshot(&definition);

    Ok(Json(SetupProfilePreviewResponse {
        profile,
        ready: true,
        summary: summarize_setup_profile_changes(&current, &target),
    }))
}

async fn apply_setup_profile(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<SetupProfileRequest>,
) -> AppResult<Json<SetupProfileApplyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let note = trim_optional(payload.note, MAX_TEMPLATE_TEXT_LEN);
    let definition = resolve_setup_profile_definition(key.as_str())?;
    let previous_state = load_setup_profile_snapshot(&state.db).await?;
    let applied_state = build_setup_profile_snapshot(&definition);

    let (run_id, actions) = apply_setup_profile_definition(
        &state,
        &definition,
        actor.as_str(),
        note.clone(),
        previous_state.clone(),
        applied_state.clone(),
    )
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "setup.profile.apply".to_string(),
            target_type: "setup_profile".to_string(),
            target_id: Some(run_id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "profile_key": definition.key,
                "profile_name": definition.name,
                "summary": summarize_setup_profile_changes(&previous_state, &applied_state),
                "actions": actions,
            }),
        },
    )
    .await;

    Ok(Json(SetupProfileApplyResponse {
        run_id,
        actor,
        profile_key: definition.key.to_string(),
        status: "applied".to_string(),
        actions,
        history_hint:
            "Use /api/v1/setup/profiles/history to review and /revert endpoint for rollback."
                .to_string(),
    }))
}

async fn list_setup_profile_history(
    State(state): State<AppState>,
    Query(query): Query<SetupProfileHistoryQuery>,
) -> AppResult<Json<SetupProfileHistoryResponse>> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_PROFILE_HISTORY_LIMIT)
        .clamp(1, MAX_PROFILE_HISTORY_LIMIT);
    let offset = query.offset.unwrap_or(0);

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM setup_operator_profile_runs")
        .fetch_one(&state.db)
        .await?;
    let items: Vec<SetupProfileHistoryRecord> = sqlx::query_as(
        "SELECT
            id,
            profile_key,
            profile_name,
            actor,
            status,
            note,
            reverted_by,
            reverted_at,
            created_at
         FROM setup_operator_profile_runs
         ORDER BY created_at DESC, id DESC
         LIMIT $1
         OFFSET $2",
    )
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(SetupProfileHistoryResponse {
        items,
        total,
        limit,
        offset,
    }))
}

async fn revert_setup_profile_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(payload): Json<SetupProfileRequest>,
) -> AppResult<Json<SetupProfileRevertResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let note = trim_optional(payload.note, MAX_TEMPLATE_TEXT_LEN);
    let run = load_setup_profile_run(&state.db, id).await?;
    if run.reverted_at.is_some() || run.status == "reverted" {
        return Err(AppError::Validation(format!(
            "setup profile run {} has already been reverted",
            run.id
        )));
    }

    let previous_state = deserialize_profile_snapshot(&run.previous_state, "previous_state")?;
    let applied_state = deserialize_profile_snapshot(&run.applied_state, "applied_state")?;

    let mut tx = state.db.begin().await?;
    restore_setup_profile_snapshot(&mut tx, actor.as_str(), &previous_state).await?;

    let reverted_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE setup_operator_profile_runs
         SET status = 'reverted',
             reverted_by = $2,
             reverted_at = NOW()
         WHERE id = $1
         RETURNING reverted_at",
    )
    .bind(run.id)
    .bind(actor.as_str())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "setup.profile.revert".to_string(),
            target_type: "setup_profile".to_string(),
            target_id: Some(run.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "profile_key": run.profile_key,
                "profile_name": run.profile_name,
                "reverted_run_id": run.id,
                "previous_state": previous_state,
                "applied_state": applied_state,
            }),
        },
    )
    .await;

    Ok(Json(SetupProfileRevertResponse {
        run_id: run.id,
        status: "reverted".to_string(),
        reverted_by: actor,
        reverted_at,
    }))
}

fn setup_profile_catalog_items() -> Vec<SetupProfileCatalogItem> {
    setup_profile_definitions()
        .into_iter()
        .map(|definition| setup_profile_catalog_item(&definition))
        .collect()
}

fn setup_profile_catalog_item(
    definition: &SetupOperatorProfileDefinition,
) -> SetupProfileCatalogItem {
    SetupProfileCatalogItem {
        key: definition.key.to_string(),
        name: definition.name.to_string(),
        description: definition.description.to_string(),
        target_scale: definition.target_scale.to_string(),
        defaults: json!({
            "identity": {
                "identity_mode": definition.identity_mode,
                "break_glass_users": parse_profile_break_glass_users(definition.break_glass_users),
            },
            "alert_policy": {
                "policy_key": PROFILE_ALERT_POLICY_KEY,
                "dedup_window_seconds": definition.alert_dedup_window_seconds,
                "ticket_priority": definition.alert_ticket_priority,
                "ticket_category": "incident",
            },
            "escalation_policy": {
                "policy_key": PROFILE_ESCALATION_POLICY_KEY,
                "near_high_minutes": definition.escalation_near_high_minutes,
                "breach_high_minutes": definition.escalation_breach_high_minutes,
                "near_medium_minutes": definition.escalation_near_medium_minutes,
                "breach_medium_minutes": definition.escalation_breach_medium_minutes,
                "escalate_to_assignee": definition.escalation_owner,
            },
            "backup_policy": {
                "policy_key": PROFILE_BACKUP_POLICY_KEY,
                "frequency": definition.backup_frequency,
                "schedule_time_utc": definition.backup_schedule_time_utc,
                "retention_days": definition.backup_retention_days,
                "destination_uri": definition.backup_destination_uri,
                "drill_frequency": definition.drill_frequency,
                "drill_weekday": definition.drill_weekday,
                "drill_time_utc": definition.drill_time_utc,
            },
        }),
    }
}

fn setup_profile_definitions() -> Vec<SetupOperatorProfileDefinition> {
    vec![
        SetupOperatorProfileDefinition {
            key: SETUP_PROFILE_SMALL_OFFICE,
            name: "SMB Small Office",
            description: "Single-site baseline with conservative ticket dedup and daily local continuity checks.",
            target_scale: "10-80 assets",
            identity_mode: "break_glass_only",
            break_glass_users: "admin,ops.emergency",
            alert_dedup_window_seconds: 1800,
            alert_ticket_priority: "high",
            escalation_near_high_minutes: 30,
            escalation_breach_high_minutes: 60,
            escalation_near_medium_minutes: 90,
            escalation_breach_medium_minutes: 180,
            escalation_owner: "ops-oncall",
            backup_frequency: "daily",
            backup_schedule_time_utc: "01:30",
            backup_retention_days: 14,
            backup_destination_uri: "file:///var/lib/cloudops/backups/smb-small-office",
            drill_frequency: "weekly",
            drill_weekday: Some(3),
            drill_time_utc: "02:30",
        },
        SetupOperatorProfileDefinition {
            key: SETUP_PROFILE_MULTI_SITE_RETAIL,
            name: "SMB Multi-Site Retail",
            description: "Multi-site default with faster alert dedup and tighter escalation budget for branch incidents.",
            target_scale: "80-800 assets",
            identity_mode: "break_glass_only",
            break_glass_users: "admin,ops.emergency,retail.lead",
            alert_dedup_window_seconds: 900,
            alert_ticket_priority: "high",
            escalation_near_high_minutes: 20,
            escalation_breach_high_minutes: 40,
            escalation_near_medium_minutes: 60,
            escalation_breach_medium_minutes: 120,
            escalation_owner: "retail-oncall",
            backup_frequency: "daily",
            backup_schedule_time_utc: "00:45",
            backup_retention_days: 21,
            backup_destination_uri: "file:///var/lib/cloudops/backups/smb-multi-site",
            drill_frequency: "weekly",
            drill_weekday: Some(2),
            drill_time_utc: "01:30",
        },
        SetupOperatorProfileDefinition {
            key: SETUP_PROFILE_REGIONAL_ENTERPRISE,
            name: "SMB Regional Enterprise",
            description: "Regional baseline with stronger retention and critical-priority alert ticket defaults.",
            target_scale: "800-5000 assets",
            identity_mode: "break_glass_only",
            break_glass_users: "admin,ops.emergency,platform.lead",
            alert_dedup_window_seconds: 600,
            alert_ticket_priority: "critical",
            escalation_near_high_minutes: 15,
            escalation_breach_high_minutes: 30,
            escalation_near_medium_minutes: 45,
            escalation_breach_medium_minutes: 90,
            escalation_owner: "regional-escalation",
            backup_frequency: "daily",
            backup_schedule_time_utc: "00:15",
            backup_retention_days: 30,
            backup_destination_uri: "file:///var/lib/cloudops/backups/smb-regional",
            drill_frequency: "weekly",
            drill_weekday: Some(1),
            drill_time_utc: "01:00",
        },
    ]
}

fn resolve_setup_profile_definition(key: &str) -> AppResult<SetupOperatorProfileDefinition> {
    let normalized = key.trim().to_ascii_lowercase();
    setup_profile_definitions()
        .into_iter()
        .find(|item| item.key == normalized)
        .ok_or_else(|| {
            AppError::Validation(format!("unsupported setup profile key '{}'", key.trim()))
        })
}

fn parse_profile_break_glass_users(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_TEMPLATE_USERS)
        .collect()
}

fn build_setup_profile_snapshot(
    definition: &SetupOperatorProfileDefinition,
) -> SetupProfileSnapshot {
    SetupProfileSnapshot {
        identity: Some(IdentityProfileState {
            identity_mode: definition.identity_mode.to_string(),
            break_glass_users: json!(parse_profile_break_glass_users(
                definition.break_glass_users
            )),
        }),
        alert_policy: Some(AlertProfileState {
            is_enabled: true,
            dedup_window_seconds: definition.alert_dedup_window_seconds,
            ticket_priority: definition.alert_ticket_priority.to_string(),
            ticket_category: "incident".to_string(),
        }),
        escalation_policy: Some(EscalationProfileState {
            is_enabled: true,
            near_high_minutes: definition.escalation_near_high_minutes,
            breach_high_minutes: definition.escalation_breach_high_minutes,
            near_medium_minutes: definition.escalation_near_medium_minutes,
            breach_medium_minutes: definition.escalation_breach_medium_minutes,
            escalate_to_assignee: definition.escalation_owner.to_string(),
        }),
        backup_policy: Some(BackupProfileState {
            frequency: definition.backup_frequency.to_string(),
            schedule_time_utc: definition.backup_schedule_time_utc.to_string(),
            schedule_weekday: if definition.backup_frequency == "weekly" {
                definition.drill_weekday
            } else {
                None
            },
            retention_days: definition.backup_retention_days,
            destination_type: "local".to_string(),
            destination_uri: definition.backup_destination_uri.to_string(),
            drill_enabled: true,
            drill_frequency: definition.drill_frequency.to_string(),
            drill_weekday: definition.drill_weekday,
            drill_time_utc: definition.drill_time_utc.to_string(),
        }),
    }
}

async fn load_setup_profile_snapshot(db: &sqlx::PgPool) -> AppResult<SetupProfileSnapshot> {
    let identity: Option<IdentityProfileState> = sqlx::query_as::<_, IdentityProfileStateRow>(
        "SELECT identity_mode, break_glass_users
         FROM setup_identity_preferences
         WHERE id = 1",
    )
    .fetch_optional(db)
    .await?
    .map(|row| IdentityProfileState {
        identity_mode: row.identity_mode,
        break_glass_users: row.break_glass_users,
    });

    let alert_policy: Option<AlertProfileState> = sqlx::query_as::<_, AlertProfileStateRow>(
        "SELECT is_enabled, dedup_window_seconds, ticket_priority, ticket_category
         FROM alert_ticket_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PROFILE_ALERT_POLICY_KEY)
    .fetch_optional(db)
    .await?
    .map(|row| AlertProfileState {
        is_enabled: row.is_enabled,
        dedup_window_seconds: row.dedup_window_seconds,
        ticket_priority: row.ticket_priority,
        ticket_category: row.ticket_category,
    });

    let escalation_policy: Option<EscalationProfileState> =
        sqlx::query_as::<_, EscalationProfileStateRow>(
            "SELECT
                is_enabled,
                near_high_minutes,
                breach_high_minutes,
                near_medium_minutes,
                breach_medium_minutes,
                escalate_to_assignee
             FROM ticket_escalation_policies
             WHERE policy_key = $1
             LIMIT 1",
        )
        .bind(PROFILE_ESCALATION_POLICY_KEY)
        .fetch_optional(db)
        .await?
        .map(|row| EscalationProfileState {
            is_enabled: row.is_enabled,
            near_high_minutes: row.near_high_minutes,
            breach_high_minutes: row.breach_high_minutes,
            near_medium_minutes: row.near_medium_minutes,
            breach_medium_minutes: row.breach_medium_minutes,
            escalate_to_assignee: row.escalate_to_assignee,
        });

    let backup_policy: Option<BackupProfileState> = sqlx::query_as::<_, BackupProfileStateRow>(
        "SELECT
            frequency,
            schedule_time_utc,
            schedule_weekday,
            retention_days,
            destination_type,
            destination_uri,
            drill_enabled,
            drill_frequency,
            drill_weekday,
            drill_time_utc
         FROM ops_backup_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PROFILE_BACKUP_POLICY_KEY)
    .fetch_optional(db)
    .await?
    .map(|row| BackupProfileState {
        frequency: row.frequency,
        schedule_time_utc: row.schedule_time_utc,
        schedule_weekday: row.schedule_weekday,
        retention_days: row.retention_days,
        destination_type: row.destination_type,
        destination_uri: row.destination_uri,
        drill_enabled: row.drill_enabled,
        drill_frequency: row.drill_frequency,
        drill_weekday: row.drill_weekday,
        drill_time_utc: row.drill_time_utc,
    });

    Ok(SetupProfileSnapshot {
        identity,
        alert_policy,
        escalation_policy,
        backup_policy,
    })
}

fn summarize_setup_profile_changes(
    before: &SetupProfileSnapshot,
    after: &SetupProfileSnapshot,
) -> Vec<SetupProfileChangeSummary> {
    let identity_before = format_identity_profile_summary(before.identity.as_ref());
    let identity_after = format_identity_profile_summary(after.identity.as_ref());
    let alert_before = format_alert_profile_summary(before.alert_policy.as_ref());
    let alert_after = format_alert_profile_summary(after.alert_policy.as_ref());
    let escalation_before = format_escalation_profile_summary(before.escalation_policy.as_ref());
    let escalation_after = format_escalation_profile_summary(after.escalation_policy.as_ref());
    let backup_before = format_backup_profile_summary(before.backup_policy.as_ref());
    let backup_after = format_backup_profile_summary(after.backup_policy.as_ref());

    vec![
        SetupProfileChangeSummary {
            domain: "identity".to_string(),
            changed: identity_before != identity_after,
            before: identity_before,
            after: identity_after,
        },
        SetupProfileChangeSummary {
            domain: "alert_policy".to_string(),
            changed: alert_before != alert_after,
            before: alert_before,
            after: alert_after,
        },
        SetupProfileChangeSummary {
            domain: "escalation_policy".to_string(),
            changed: escalation_before != escalation_after,
            before: escalation_before,
            after: escalation_after,
        },
        SetupProfileChangeSummary {
            domain: "backup_policy".to_string(),
            changed: backup_before != backup_after,
            before: backup_before,
            after: backup_after,
        },
    ]
}

fn format_identity_profile_summary(state: Option<&IdentityProfileState>) -> String {
    let Some(state) = state else {
        return "not configured".to_string();
    };
    let users = state
        .break_glass_users
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| "-".to_string());
    format!("mode={}, break_glass_users={users}", state.identity_mode)
}

fn format_alert_profile_summary(state: Option<&AlertProfileState>) -> String {
    let Some(state) = state else {
        return "not configured".to_string();
    };
    format!(
        "enabled={}, dedup={}s, priority={}, category={}",
        state.is_enabled, state.dedup_window_seconds, state.ticket_priority, state.ticket_category
    )
}

fn format_escalation_profile_summary(state: Option<&EscalationProfileState>) -> String {
    let Some(state) = state else {
        return "not configured".to_string();
    };
    format!(
        "enabled={}, high={}/{}, medium={}/{}, owner={}",
        state.is_enabled,
        state.near_high_minutes,
        state.breach_high_minutes,
        state.near_medium_minutes,
        state.breach_medium_minutes,
        state.escalate_to_assignee
    )
}

fn format_backup_profile_summary(state: Option<&BackupProfileState>) -> String {
    let Some(state) = state else {
        return "not configured".to_string();
    };
    format!(
        "freq={}, schedule={}({}), retention={}d, destination={} {}, drill={} {}({})",
        state.frequency,
        state.schedule_time_utc,
        state
            .schedule_weekday
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        state.retention_days,
        state.destination_type,
        state.destination_uri,
        state.drill_frequency,
        state.drill_time_utc,
        state
            .drill_weekday
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    )
}

async fn apply_setup_profile_definition(
    state: &AppState,
    definition: &SetupOperatorProfileDefinition,
    actor: &str,
    note: Option<String>,
    previous_state: SetupProfileSnapshot,
    applied_state: SetupProfileSnapshot,
) -> AppResult<(i64, Vec<SetupProfileApplyAction>)> {
    let mut tx = state.db.begin().await?;

    let identity = applied_state
        .identity
        .as_ref()
        .ok_or_else(|| AppError::Validation("profile identity state is missing".to_string()))?;
    let alert_policy = applied_state
        .alert_policy
        .as_ref()
        .ok_or_else(|| AppError::Validation("profile alert policy state is missing".to_string()))?;
    let escalation_policy = applied_state.escalation_policy.as_ref().ok_or_else(|| {
        AppError::Validation("profile escalation policy state is missing".to_string())
    })?;
    let backup_policy = applied_state.backup_policy.as_ref().ok_or_else(|| {
        AppError::Validation("profile backup policy state is missing".to_string())
    })?;

    let actions = vec![
        upsert_identity_profile_state(&mut tx, actor, identity).await?,
        upsert_alert_profile_state(&mut tx, actor, alert_policy).await?,
        upsert_escalation_profile_state(&mut tx, actor, escalation_policy).await?,
        upsert_backup_profile_state(&mut tx, actor, backup_policy).await?,
    ];

    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO setup_operator_profile_runs (
            profile_key,
            profile_name,
            actor,
            status,
            note,
            previous_state,
            applied_state
         )
         VALUES ($1, $2, $3, 'applied', $4, $5, $6)
         RETURNING id",
    )
    .bind(definition.key)
    .bind(definition.name)
    .bind(actor)
    .bind(note)
    .bind(serialize_profile_snapshot(&previous_state)?)
    .bind(serialize_profile_snapshot(&applied_state)?)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((run_id, actions))
}

fn serialize_profile_snapshot(snapshot: &SetupProfileSnapshot) -> AppResult<Value> {
    serde_json::to_value(snapshot)
        .map_err(|err| AppError::Validation(format!("failed to serialize profile snapshot: {err}")))
}

fn deserialize_profile_snapshot(value: &Value, field: &str) -> AppResult<SetupProfileSnapshot> {
    serde_json::from_value(value.clone())
        .map_err(|err| AppError::Validation(format!("invalid {field} json payload: {err}")))
}

async fn upsert_identity_profile_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &str,
    state: &IdentityProfileState,
) -> AppResult<SetupProfileApplyAction> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM setup_identity_preferences)")
            .fetch_one(&mut **tx)
            .await?;

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
    .bind(state.identity_mode.as_str())
    .bind(state.break_glass_users.clone())
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    Ok(SetupProfileApplyAction {
        action_key: "profile.identity".to_string(),
        outcome: if exists {
            "updated".to_string()
        } else {
            "created".to_string()
        },
        detail: format_identity_profile_summary(Some(state)),
    })
}

async fn upsert_alert_profile_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &str,
    state: &AlertProfileState,
) -> AppResult<SetupProfileApplyAction> {
    let existing_id: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM alert_ticket_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PROFILE_ALERT_POLICY_KEY)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE alert_ticket_policies
             SET name = $2,
                 description = $3,
                 is_system = TRUE,
                 is_enabled = $4,
                 match_status = 'open',
                 dedup_window_seconds = $5,
                 ticket_priority = $6,
                 ticket_category = $7,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind("Operator Profile Default Alert Policy")
        .bind("Managed by setup profile apply flow")
        .bind(state.is_enabled)
        .bind(state.dedup_window_seconds)
        .bind(state.ticket_priority.as_str())
        .bind(state.ticket_category.as_str())
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO alert_ticket_policies (
                policy_key,
                name,
                description,
                is_system,
                is_enabled,
                match_source,
                match_severity,
                match_site,
                match_department,
                match_status,
                dedup_window_seconds,
                ticket_priority,
                ticket_category,
                workflow_template_id,
                created_by
             )
             VALUES (
                $1,
                $2,
                $3,
                TRUE,
                $4,
                NULL,
                NULL,
                NULL,
                NULL,
                'open',
                $5,
                $6,
                $7,
                NULL,
                $8
             )",
        )
        .bind(PROFILE_ALERT_POLICY_KEY)
        .bind("Operator Profile Default Alert Policy")
        .bind("Managed by setup profile apply flow")
        .bind(state.is_enabled)
        .bind(state.dedup_window_seconds)
        .bind(state.ticket_priority.as_str())
        .bind(state.ticket_category.as_str())
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    }

    Ok(SetupProfileApplyAction {
        action_key: "profile.alert_policy".to_string(),
        outcome: if existing_id.is_some() {
            "updated".to_string()
        } else {
            "created".to_string()
        },
        detail: format_alert_profile_summary(Some(state)),
    })
}

async fn upsert_escalation_profile_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &str,
    state: &EscalationProfileState,
) -> AppResult<SetupProfileApplyAction> {
    let existing_id: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ticket_escalation_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PROFILE_ESCALATION_POLICY_KEY)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE ticket_escalation_policies
             SET name = $2,
                 is_enabled = $3,
                 near_high_minutes = $4,
                 breach_high_minutes = $5,
                 near_medium_minutes = $6,
                 breach_medium_minutes = $7,
                 escalate_to_assignee = $8,
                 updated_by = $9,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind("Default Ticket SLA Policy")
        .bind(state.is_enabled)
        .bind(state.near_high_minutes)
        .bind(state.breach_high_minutes)
        .bind(state.near_medium_minutes)
        .bind(state.breach_medium_minutes)
        .bind(state.escalate_to_assignee.as_str())
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO ticket_escalation_policies (
                policy_key,
                name,
                is_enabled,
                near_critical_minutes,
                breach_critical_minutes,
                near_high_minutes,
                breach_high_minutes,
                near_medium_minutes,
                breach_medium_minutes,
                near_low_minutes,
                breach_low_minutes,
                escalate_to_assignee,
                updated_by
             )
             VALUES (
                $1,
                $2,
                $3,
                30,
                60,
                $4,
                $5,
                $6,
                $7,
                240,
                480,
                $8,
                $9
             )",
        )
        .bind(PROFILE_ESCALATION_POLICY_KEY)
        .bind("Default Ticket SLA Policy")
        .bind(state.is_enabled)
        .bind(state.near_high_minutes)
        .bind(state.breach_high_minutes)
        .bind(state.near_medium_minutes)
        .bind(state.breach_medium_minutes)
        .bind(state.escalate_to_assignee.as_str())
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    }

    Ok(SetupProfileApplyAction {
        action_key: "profile.escalation_policy".to_string(),
        outcome: if existing_id.is_some() {
            "updated".to_string()
        } else {
            "created".to_string()
        },
        detail: format_escalation_profile_summary(Some(state)),
    })
}

async fn upsert_backup_profile_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &str,
    state: &BackupProfileState,
) -> AppResult<SetupProfileApplyAction> {
    let existing_id: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_backup_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PROFILE_BACKUP_POLICY_KEY)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(id) = existing_id {
        sqlx::query(
            "UPDATE ops_backup_policies
             SET name = $2,
                 frequency = $3,
                 schedule_time_utc = $4,
                 schedule_weekday = $5,
                 retention_days = $6,
                 destination_type = $7,
                 destination_uri = $8,
                 destination_validated = TRUE,
                 drill_enabled = $9,
                 drill_frequency = $10,
                 drill_weekday = $11,
                 drill_time_utc = $12,
                 next_backup_at = CASE
                     WHEN $3 = 'daily' THEN NOW() + INTERVAL '1 day'
                     ELSE NOW() + INTERVAL '7 day'
                 END,
                 next_drill_at = CASE
                     WHEN $9 = FALSE THEN NULL
                     WHEN $10 = 'weekly' THEN NOW() + INTERVAL '7 day'
                     WHEN $10 = 'monthly' THEN NOW() + INTERVAL '30 day'
                     ELSE NOW() + INTERVAL '90 day'
                 END,
                 updated_by = $13,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind("Operator Profile Default Backup Policy")
        .bind(state.frequency.as_str())
        .bind(state.schedule_time_utc.as_str())
        .bind(state.schedule_weekday)
        .bind(state.retention_days)
        .bind(state.destination_type.as_str())
        .bind(state.destination_uri.as_str())
        .bind(state.drill_enabled)
        .bind(state.drill_frequency.as_str())
        .bind(state.drill_weekday)
        .bind(state.drill_time_utc.as_str())
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO ops_backup_policies (
                policy_key,
                name,
                frequency,
                schedule_time_utc,
                schedule_weekday,
                retention_days,
                destination_type,
                destination_uri,
                destination_validated,
                drill_enabled,
                drill_frequency,
                drill_weekday,
                drill_time_utc,
                next_backup_at,
                next_drill_at,
                updated_by
             )
             VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                TRUE,
                $9,
                $10,
                $11,
                $12,
                CASE
                    WHEN $3 = 'daily' THEN NOW() + INTERVAL '1 day'
                    ELSE NOW() + INTERVAL '7 day'
                END,
                CASE
                    WHEN $9 = FALSE THEN NULL
                    WHEN $10 = 'weekly' THEN NOW() + INTERVAL '7 day'
                    WHEN $10 = 'monthly' THEN NOW() + INTERVAL '30 day'
                    ELSE NOW() + INTERVAL '90 day'
                END,
                $13
             )",
        )
        .bind(PROFILE_BACKUP_POLICY_KEY)
        .bind("Operator Profile Default Backup Policy")
        .bind(state.frequency.as_str())
        .bind(state.schedule_time_utc.as_str())
        .bind(state.schedule_weekday)
        .bind(state.retention_days)
        .bind(state.destination_type.as_str())
        .bind(state.destination_uri.as_str())
        .bind(state.drill_enabled)
        .bind(state.drill_frequency.as_str())
        .bind(state.drill_weekday)
        .bind(state.drill_time_utc.as_str())
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    }

    Ok(SetupProfileApplyAction {
        action_key: "profile.backup_policy".to_string(),
        outcome: if existing_id.is_some() {
            "updated".to_string()
        } else {
            "created".to_string()
        },
        detail: format_backup_profile_summary(Some(state)),
    })
}

async fn restore_setup_profile_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &str,
    snapshot: &SetupProfileSnapshot,
) -> AppResult<()> {
    if let Some(identity) = snapshot.identity.as_ref() {
        upsert_identity_profile_state(tx, actor, identity).await?;
    } else {
        sqlx::query("DELETE FROM setup_identity_preferences WHERE id = 1")
            .execute(&mut **tx)
            .await?;
    }

    if let Some(alert_policy) = snapshot.alert_policy.as_ref() {
        upsert_alert_profile_state(tx, actor, alert_policy).await?;
    } else {
        sqlx::query("DELETE FROM alert_ticket_policies WHERE policy_key = $1")
            .bind(PROFILE_ALERT_POLICY_KEY)
            .execute(&mut **tx)
            .await?;
    }

    if let Some(escalation_policy) = snapshot.escalation_policy.as_ref() {
        upsert_escalation_profile_state(tx, actor, escalation_policy).await?;
    } else {
        sqlx::query("DELETE FROM ticket_escalation_policies WHERE policy_key = $1")
            .bind(PROFILE_ESCALATION_POLICY_KEY)
            .execute(&mut **tx)
            .await?;
    }

    if let Some(backup_policy) = snapshot.backup_policy.as_ref() {
        upsert_backup_profile_state(tx, actor, backup_policy).await?;
    } else {
        sqlx::query("DELETE FROM ops_backup_policies WHERE policy_key = $1")
            .bind(PROFILE_BACKUP_POLICY_KEY)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn load_setup_profile_run(db: &sqlx::PgPool, id: i64) -> AppResult<SetupProfileRunRow> {
    if id <= 0 {
        return Err(AppError::Validation(
            "profile run id must be a positive integer".to_string(),
        ));
    }
    let run: Option<SetupProfileRunRow> = sqlx::query_as(
        "SELECT
            id,
            profile_key,
            profile_name,
            status,
            previous_state,
            applied_state,
            reverted_at
         FROM setup_operator_profile_runs
         WHERE id = $1
         LIMIT 1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    run.ok_or_else(|| AppError::NotFound(format!("setup profile run {id} not found")))
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

async fn collect_setup_preflight_checks(state: &AppState) -> Vec<SetupCheckItem> {
    vec![
        check_database(state.db.clone()).await,
        check_rbac_mode(state.rbac_enabled),
        check_oidc_settings(state),
        check_monitoring_secret_settings(state),
        check_workflow_policy_mode(state),
    ]
}

async fn collect_setup_checklist_checks(state: &AppState) -> Vec<SetupCheckItem> {
    vec![
        SetupCheckItem {
            key: "api-service".to_string(),
            title: "API Service".to_string(),
            status: SetupCheckStatus::Pass,
            critical: true,
            message: "API endpoint is reachable because checklist request succeeded.".to_string(),
            remediation: "If this check fails in other environments, start API service and verify API_HOST/API_PORT.".to_string(),
        },
        check_database(state.db.clone()).await.with_key("database"),
        tcp_endpoint_check(
            "web-console",
            "Web Console",
            &read_addr("SETUP_WEB_ADDR", DEFAULT_WEB_ADDR),
            true,
            "Ensure web console is running (for local stack: bash scripts/install.sh or docker compose up web).",
        )
        .await,
        tcp_endpoint_check(
            "redis",
            "Redis",
            &read_addr("SETUP_REDIS_ADDR", DEFAULT_REDIS_ADDR),
            false,
            "Ensure redis service is healthy and accessible from API host.",
        )
        .await,
        tcp_endpoint_check(
            "opensearch",
            "OpenSearch",
            &read_addr("SETUP_OPENSEARCH_ADDR", DEFAULT_OPENSEARCH_ADDR),
            false,
            "Ensure opensearch service is healthy and accessible from API host.",
        )
        .await,
        tcp_endpoint_check(
            "minio",
            "MinIO",
            &read_addr("SETUP_MINIO_ADDR", DEFAULT_MINIO_ADDR),
            false,
            "Ensure minio service is healthy and accessible from API host.",
        )
        .await,
        tcp_endpoint_check(
            "zabbix-server",
            "Zabbix Server",
            &read_addr("SETUP_ZABBIX_ADDR", DEFAULT_ZABBIX_ADDR),
            false,
            "Ensure zabbix server is running and the trapper/listener port is reachable.",
        )
        .await,
        check_monitoring_source_seed(state.db.clone()).await,
        check_alert_policy_templates(state.db.clone()).await,
        check_setup_template_baseline(state.db.clone()).await,
        tcp_endpoint_check(
            "api-port",
            "API Port",
            &read_addr("SETUP_API_ADDR", DEFAULT_API_ADDR),
            true,
            "Ensure API bind address is reachable on the expected host and port.",
        )
        .await,
    ]
}

fn summarize_activation_checklist_item(
    item_key: &str,
    title: &str,
    ready_reason: &str,
    response: &SetupChecklistResponse,
    recommended_action: Option<SetupActivationRecommendedAction>,
) -> SetupActivationItem {
    let (status, summary, reason) = if response.summary.critical_failed > 0 {
        (
            "blocking".to_string(),
            format!(
                "{} critical checks are blocking activation.",
                response.summary.critical_failed
            ),
            response
                .checks
                .iter()
                .find(|item| item.critical && matches!(item.status, SetupCheckStatus::Fail))
                .map(|item| item.remediation.clone())
                .unwrap_or_else(|| "Resolve blocking setup checks before continuing.".to_string()),
        )
    } else if response.summary.failed > 0 || response.summary.warned > 0 {
        (
            "warning".to_string(),
            format!(
                "{} checks still need follow-up.",
                response.summary.failed + response.summary.warned
            ),
            response
                .checks
                .iter()
                .find(|item| matches!(item.status, SetupCheckStatus::Fail | SetupCheckStatus::Warn))
                .map(|item| item.remediation.clone())
                .unwrap_or_else(|| "Review setup checks with warnings before wider rollout.".to_string()),
        )
    } else {
        (
            "ready".to_string(),
            "Checks are ready for activation.".to_string(),
            ready_reason.to_string(),
        )
    };

    SetupActivationItem {
        item_key: item_key.to_string(),
        title: title.to_string(),
        status,
        summary,
        reason,
        recommended_action,
        evidence: json!({
            "category": response.category,
            "summary": {
                "total": response.summary.total,
                "passed": response.summary.passed,
                "warned": response.summary.warned,
                "failed": response.summary.failed,
                "critical_failed": response.summary.critical_failed,
                "ready": response.summary.ready,
            }
        }),
    }
}

fn build_activation_profile_item(
    asset_count: i64,
    profile_run_count: i64,
    recommended_profile_key: &'static str,
) -> SetupActivationItem {
    let (status, summary, reason) = if profile_run_count <= 0 {
        (
            "warning".to_string(),
            "No operator starter profile has been applied yet.".to_string(),
            format!(
                "Use the recommended SMB starter profile '{}' to avoid blank-page setup work.",
                recommended_profile_key
            ),
        )
    } else {
        (
            "ready".to_string(),
            format!(
                "{} setup profile run(s) have already been applied.",
                profile_run_count
            ),
            "An operator starter profile has already seeded the environment baseline."
                .to_string(),
        )
    };

    SetupActivationItem {
        item_key: "operator_profile".to_string(),
        title: "Operator starter profile".to_string(),
        status,
        summary,
        reason,
        recommended_action: Some(link_activation_action(
            "setup.activation.profile.review",
            "Review starter profile",
            "Open the setup wizard profile presets and compare the recommended SMB baseline.",
            ACTIVATION_SETUP_HREF,
            Some(recommended_profile_key),
        )),
        evidence: json!({
            "asset_count": asset_count,
            "profile_run_count": profile_run_count,
            "recommended_profile_key": recommended_profile_key,
        }),
    }
}

fn link_activation_action(
    action_key: &str,
    label: &str,
    description: &str,
    href: &str,
    profile_key: Option<&str>,
) -> SetupActivationRecommendedAction {
    SetupActivationRecommendedAction {
        action_key: action_key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        action_type: "link".to_string(),
        href: Some(href.to_string()),
        requires_write: false,
        auto_applicable: false,
        profile_key: profile_key.map(str::to_string),
    }
}

fn recommend_setup_profile_key(asset_count: i64) -> &'static str {
    if asset_count > 800 {
        SETUP_PROFILE_REGIONAL_ENTERPRISE
    } else if asset_count > 80 {
        SETUP_PROFILE_MULTI_SITE_RETAIL
    } else {
        SETUP_PROFILE_SMALL_OFFICE
    }
}

fn summarize_activation_items(items: &[SetupActivationItem]) -> SetupActivationSummary {
    let mut summary = SetupActivationSummary {
        total: items.len(),
        ready: 0,
        warning: 0,
        blocking: 0,
    };

    for item in items {
        match item.status.as_str() {
            "ready" => summary.ready += 1,
            "warning" => summary.warning += 1,
            "blocking" => summary.blocking += 1,
            _ => {}
        }
    }

    summary
}

fn derive_activation_overall_status(summary: &SetupActivationSummary) -> String {
    if summary.blocking > 0 {
        "blocking".to_string()
    } else if summary.warning > 0 {
        "warning".to_string()
    } else {
        "ready".to_string()
    }
}

fn select_next_activation_item_key(items: &[SetupActivationItem]) -> Option<String> {
    items.iter()
        .find(|item| item.status == "blocking")
        .or_else(|| items.iter().find(|item| item.status == "warning"))
        .map(|item| item.item_key.clone())
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
        SetupActivationItem, derive_activation_overall_status, parse_rollback_hints,
        recommend_setup_profile_key, select_next_activation_item_key,
        summarize_activation_items, validate_identity_template_params,
        validate_monitoring_template_params, validate_notification_template_params,
    };

    fn json_params(value: Value) -> JsonMap<String, Value> {
        value.as_object().cloned().expect("object")
    }

    #[test]
    fn activation_summary_counts_and_derives_overall() {
        let summary = summarize_activation_items(&[
            SetupActivationItem {
                item_key: "environment".to_string(),
                title: "Environment".to_string(),
                status: "ready".to_string(),
                summary: "ok".to_string(),
                reason: "ok".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            SetupActivationItem {
                item_key: "profile".to_string(),
                title: "Profile".to_string(),
                status: "warning".to_string(),
                summary: "warn".to_string(),
                reason: "warn".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            SetupActivationItem {
                item_key: "integration".to_string(),
                title: "Integration".to_string(),
                status: "blocking".to_string(),
                summary: "block".to_string(),
                reason: "block".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
        ]);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.ready, 1);
        assert_eq!(summary.warning, 1);
        assert_eq!(summary.blocking, 1);
        assert_eq!(derive_activation_overall_status(&summary), "blocking");
    }

    #[test]
    fn activation_next_step_prefers_blocking_then_warning() {
        let items = vec![
            SetupActivationItem {
                item_key: "environment".to_string(),
                title: "Environment".to_string(),
                status: "ready".to_string(),
                summary: "ok".to_string(),
                reason: "ok".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            SetupActivationItem {
                item_key: "profile".to_string(),
                title: "Profile".to_string(),
                status: "warning".to_string(),
                summary: "warn".to_string(),
                reason: "warn".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            SetupActivationItem {
                item_key: "integration".to_string(),
                title: "Integration".to_string(),
                status: "blocking".to_string(),
                summary: "block".to_string(),
                reason: "block".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
        ];

        assert_eq!(
            select_next_activation_item_key(&items),
            Some("integration".to_string())
        );
    }

    #[test]
    fn setup_profile_recommendation_tracks_asset_size() {
        assert_eq!(recommend_setup_profile_key(0), "smb-small-office");
        assert_eq!(recommend_setup_profile_key(80), "smb-small-office");
        assert_eq!(recommend_setup_profile_key(81), "smb-multi-site-retail");
        assert_eq!(recommend_setup_profile_key(801), "smb-regional-enterprise");
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
