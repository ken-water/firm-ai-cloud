use std::collections::BTreeSet;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use tracing::warn;
use uuid::Uuid;

use crate::{
    alerts::append_alert_remediation_timeline,
    audit::{actor_from_headers, write_from_headers_best_effort},
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_PLAYBOOK_KEY_LEN: usize = 64;
const MAX_CATEGORY_LEN: usize = 64;
const MAX_ASSET_REF_LEN: usize = 128;
const MAX_ACTOR_LEN: usize = 128;
const MAX_QUERY_LEN: usize = 128;
const MAX_PARAM_FIELD_KEY_LEN: usize = 64;
const MAX_PARAM_FIELD_TYPE_LEN: usize = 32;
const MAX_PARAM_FIELD_OPTIONS: usize = 128;
const MAX_PARAM_FIELDS: usize = 64;
const MAX_PLAN_STEPS: usize = 128;
const MAX_STEP_TEXT_LEN: usize = 256;
const MAX_EXECUTION_LIMIT: u32 = 200;
const DEFAULT_EXECUTION_LIMIT: u32 = 50;
const CONFIRMATION_TTL_MINUTES: i64 = 120;
const MAX_POLICY_TIMEZONE_LEN: usize = 64;
const MAX_POLICY_NOTE_LEN: usize = 1024;
const MAX_WINDOW_LABEL_LEN: usize = 128;
const MAX_OVERRIDE_REASON_LEN: usize = 1024;
const MAX_APPROVAL_NOTE_LEN: usize = 1024;
const PLAYBOOK_POLICY_DEFAULT_KEY: &str = "global";
const APPROVAL_TTL_MINUTES: i64 = 120;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playbooks", get(list_playbooks))
        .route(
            "/playbooks/policy",
            get(get_playbook_execution_policy).put(update_playbook_execution_policy),
        )
        .route("/playbooks/approvals", get(list_playbook_approval_requests))
        .route(
            "/playbooks/approvals/{id}/approve",
            axum::routing::post(approve_playbook_approval_request),
        )
        .route(
            "/playbooks/approvals/{id}/reject",
            axum::routing::post(reject_playbook_approval_request),
        )
        .route("/playbooks/executions", get(list_playbook_executions))
        .route("/playbooks/executions/{id}", get(get_playbook_execution))
        .route(
            "/playbooks/executions/{id}/replay",
            axum::routing::post(replay_playbook_execution),
        )
        .route(
            "/playbooks/{key}/approval-request",
            axum::routing::post(request_playbook_approval),
        )
        .route("/playbooks/{key}", get(get_playbook_detail))
        .route(
            "/playbooks/{key}/dry-run",
            axum::routing::post(dry_run_playbook),
        )
        .route(
            "/playbooks/{key}/execute",
            axum::routing::post(execute_playbook),
        )
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookCatalogItem {
    id: i64,
    key: String,
    name: String,
    category: String,
    risk_level: String,
    params: Value,
    description: Option<String>,
    requires_confirmation: bool,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListPlaybooksResponse {
    items: Vec<PlaybookCatalogItem>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct PlaybookDetailResponse {
    id: i64,
    key: String,
    name: String,
    description: Option<String>,
    category: String,
    risk_level: String,
    requires_confirmation: bool,
    params: Value,
    execution_plan: Value,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookExecutionListItem {
    id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_verified: bool,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ListPlaybookExecutionsResponse {
    items: Vec<PlaybookExecutionListItem>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookExecutionDetail {
    id: i64,
    playbook_id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_verified: bool,
    confirmed_at: Option<DateTime<Utc>>,
    params: Value,
    planned_steps: Value,
    result: Value,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    expires_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PlaybookDryRunResponse {
    execution: PlaybookExecutionDetail,
    risk_summary: DryRunRiskSummary,
    confirmation: Option<DryRunConfirmationChallenge>,
}

#[derive(Debug, Serialize)]
struct DryRunRiskSummary {
    risk_level: String,
    requires_confirmation: bool,
    ttl_minutes: i64,
    summary: String,
}

#[derive(Debug, Serialize)]
struct DryRunConfirmationChallenge {
    token: String,
    expires_at: DateTime<Utc>,
    instruction: String,
}

#[derive(Debug, Serialize)]
struct ReplayExecutionResponse {
    mode: String,
    source_execution_id: i64,
    execution: PlaybookExecutionDetail,
    note: String,
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookApprovalRequestRecord {
    id: i64,
    dry_run_execution_id: i64,
    playbook_id: i64,
    playbook_key: String,
    requester: String,
    request_note: Option<String>,
    status: String,
    approver: Option<String>,
    approver_note: Option<String>,
    approval_token: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListPlaybookApprovalRequestsResponse {
    items: Vec<PlaybookApprovalRequestRecord>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize, Clone, Deserialize)]
struct PlaybookMaintenanceWindow {
    day_of_week: u8,
    start: String,
    end: String,
    label: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlaybookExecutionPolicyView {
    policy_key: String,
    timezone_name: String,
    maintenance_windows: Vec<PlaybookMaintenanceWindow>,
    change_freeze_enabled: bool,
    override_requires_reason: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PlaybookExecutionPolicyRuntime {
    timezone_now: String,
    in_maintenance_window: bool,
    next_allowed_at: Option<String>,
    blocked_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlaybookExecutionPolicyResponse {
    policy: PlaybookExecutionPolicyView,
    runtime: PlaybookExecutionPolicyRuntime,
}

#[derive(Debug, Deserialize)]
struct UpdatePlaybookExecutionPolicyRequest {
    timezone_name: Option<String>,
    maintenance_windows: Option<Vec<PlaybookMaintenanceWindow>>,
    change_freeze_enabled: Option<bool>,
    override_requires_reason: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListPlaybooksQuery {
    category: Option<String>,
    risk_level: Option<String>,
    is_enabled: Option<bool>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct PlaybookExecutionListQuery {
    playbook_key: Option<String>,
    mode: Option<String>,
    status: Option<String>,
    actor: Option<String>,
    asset_ref: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct PlaybookApprovalRequestListQuery {
    playbook_key: Option<String>,
    status: Option<String>,
    requester: Option<String>,
    approver: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PlaybookRunRequest {
    params: Option<Value>,
    asset_ref: Option<String>,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlaybookExecuteRequest {
    params: Option<Value>,
    asset_ref: Option<String>,
    dry_run_id: Option<i64>,
    confirmation_token: Option<String>,
    approval_id: Option<i64>,
    approval_token: Option<String>,
    maintenance_override_reason: Option<String>,
    maintenance_override_confirmed: Option<bool>,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlaybookApprovalRequestInput {
    dry_run_id: i64,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaybookApprovalDecisionRequest {
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReplayExecutionRequest {
    mode: Option<String>,
    dry_run_id: Option<i64>,
    confirmation_token: Option<String>,
    approval_id: Option<i64>,
    approval_token: Option<String>,
    maintenance_override_reason: Option<String>,
    maintenance_override_confirmed: Option<bool>,
}

#[derive(Debug, Clone, FromRow)]
struct PlaybookRecord {
    id: i64,
    key: String,
    name: String,
    description: Option<String>,
    category: String,
    risk_level: String,
    requires_confirmation: bool,
    params: Value,
    execution_plan: Value,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DryRunConfirmationRecord {
    id: i64,
    playbook_id: i64,
    actor: String,
    confirmation_token: Option<String>,
    confirmation_required: bool,
    expires_at: Option<DateTime<Utc>>,
    mode: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct DryRunApprovalSourceRecord {
    id: i64,
    playbook_id: i64,
    playbook_key: String,
    actor: String,
    mode: String,
    status: String,
    confirmation_required: bool,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct ApprovalConsumeRecord {
    id: i64,
    dry_run_execution_id: i64,
    playbook_id: i64,
    requester: String,
    status: String,
    approver: Option<String>,
    approval_token: Option<String>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct PlaybookExecutionPolicyRow {
    policy_key: String,
    timezone_name: String,
    maintenance_windows: Value,
    change_freeze_enabled: bool,
    override_requires_reason: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct PlaybookExecutionPolicy {
    policy_key: String,
    timezone_name: String,
    timezone: Tz,
    maintenance_windows: Vec<PlaybookMaintenanceWindow>,
    change_freeze_enabled: bool,
    override_requires_reason: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MaintenanceOverrideInput {
    reason: Option<String>,
    confirmed: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookParameterSchema {
    #[serde(default)]
    fields: Vec<PlaybookParameterField>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookParameterField {
    key: String,
    #[serde(rename = "type")]
    field_type: String,
    #[serde(default)]
    required: bool,
    min: Option<f64>,
    max: Option<f64>,
    max_length: Option<usize>,
    options: Option<Vec<String>>,
    default: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookExecutionPlan {
    #[serde(default)]
    steps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayMode {
    DryRun,
    Execute,
}

impl ReplayMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::Execute => "execute",
        }
    }
}

async fn list_playbooks(
    State(state): State<AppState>,
    Query(query): Query<ListPlaybooksQuery>,
) -> AppResult<Json<ListPlaybooksResponse>> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let category = trim_optional(query.category, MAX_CATEGORY_LEN);
    let risk_level = normalize_optional_risk_level(query.risk_level)?;
    let is_enabled = query.is_enabled.unwrap_or(true);
    let query_text = trim_optional(query.query, MAX_QUERY_LEN);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM workflow_playbooks p WHERE 1=1");
    append_playbook_filters(
        &mut count_builder,
        category.clone(),
        risk_level.clone(),
        Some(is_enabled),
        query_text.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id,
                playbook_key AS key,
                name,
                category,
                risk_level,
                parameter_schema AS params,
                description,
                requires_confirmation,
                rbac_hint,
                is_enabled,
                is_system,
                updated_at
         FROM workflow_playbooks p
         WHERE 1=1",
    );
    append_playbook_filters(
        &mut list_builder,
        category,
        risk_level,
        Some(is_enabled),
        query_text,
    );
    list_builder
        .push(" ORDER BY p.category ASC, p.risk_level DESC, p.name ASC, p.id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<PlaybookCatalogItem> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListPlaybooksResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_playbook_detail(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> AppResult<Json<PlaybookDetailResponse>> {
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;

    Ok(Json(PlaybookDetailResponse {
        id: playbook.id,
        key: playbook.key,
        name: playbook.name,
        description: playbook.description,
        category: playbook.category,
        risk_level: playbook.risk_level,
        requires_confirmation: playbook.requires_confirmation,
        params: playbook.params,
        execution_plan: playbook.execution_plan,
        rbac_hint: playbook.rbac_hint,
        is_enabled: playbook.is_enabled,
        is_system: playbook.is_system,
        created_at: playbook.created_at,
        updated_at: playbook.updated_at,
    }))
}

async fn get_playbook_execution_policy(
    State(state): State<AppState>,
) -> AppResult<Json<PlaybookExecutionPolicyResponse>> {
    let policy = load_or_init_playbook_execution_policy(&state.db).await?;
    Ok(Json(build_policy_response(&policy, Utc::now())))
}

async fn update_playbook_execution_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePlaybookExecutionPolicyRequest>,
) -> AppResult<Json<PlaybookExecutionPolicyResponse>> {
    let actor = resolve_actor(&headers);
    let current = load_or_init_playbook_execution_policy(&state.db).await?;

    let timezone_name = payload
        .timezone_name
        .map(normalize_timezone_name)
        .transpose()?
        .unwrap_or_else(|| current.timezone_name.clone());
    let timezone = parse_timezone(timezone_name.as_str())?;

    let maintenance_windows = payload
        .maintenance_windows
        .map(normalize_maintenance_windows)
        .transpose()?
        .unwrap_or_else(|| current.maintenance_windows.clone());
    let change_freeze_enabled = payload
        .change_freeze_enabled
        .unwrap_or(current.change_freeze_enabled);
    let override_requires_reason = payload
        .override_requires_reason
        .unwrap_or(current.override_requires_reason);
    let note = trim_optional(payload.note, MAX_POLICY_NOTE_LEN);

    let updated_row: PlaybookExecutionPolicyRow = sqlx::query_as(
        "UPDATE workflow_playbook_execution_policies
         SET timezone_name = $2,
             maintenance_windows = $3,
             change_freeze_enabled = $4,
             override_requires_reason = $5,
             updated_by = $6,
             updated_at = NOW()
         WHERE policy_key = $1
         RETURNING policy_key, timezone_name, maintenance_windows, change_freeze_enabled,
                   override_requires_reason, updated_by, updated_at",
    )
    .bind(PLAYBOOK_POLICY_DEFAULT_KEY)
    .bind(&timezone_name)
    .bind(serde_json::to_value(&maintenance_windows).map_err(|err| {
        AppError::Validation(format!(
            "failed to serialize maintenance_windows policy update: {err}"
        ))
    })?)
    .bind(change_freeze_enabled)
    .bind(override_requires_reason)
    .bind(&actor)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.policy.update",
        "workflow_playbook_policy",
        Some(updated_row.policy_key.clone()),
        "success",
        note.clone(),
        json!({
            "timezone_name": timezone_name,
            "maintenance_window_count": maintenance_windows.len(),
            "change_freeze_enabled": change_freeze_enabled,
            "override_requires_reason": override_requires_reason,
        }),
    )
    .await;

    let updated = parse_playbook_execution_policy_row(updated_row, Some(timezone))?;
    Ok(Json(build_policy_response(&updated, Utc::now())))
}

async fn list_playbook_approval_requests(
    State(state): State<AppState>,
    Query(query): Query<PlaybookApprovalRequestListQuery>,
) -> AppResult<Json<ListPlaybookApprovalRequestsResponse>> {
    let playbook_key = query.playbook_key.map(normalize_playbook_key).transpose()?;
    let status = normalize_optional_approval_status(query.status)?;
    let requester = trim_optional(query.requester, MAX_ACTOR_LEN);
    let approver = trim_optional(query.approver, MAX_ACTOR_LEN);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM workflow_playbook_approval_requests a WHERE 1=1");
    append_approval_filters(
        &mut count_builder,
        playbook_key.clone(),
        status.clone(),
        requester.clone(),
        approver.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                created_at, updated_at
         FROM workflow_playbook_approval_requests a
         WHERE 1=1",
    );
    append_approval_filters(
        &mut list_builder,
        playbook_key,
        status,
        requester,
        approver,
    );
    list_builder
        .push(" ORDER BY a.created_at DESC, a.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<PlaybookApprovalRequestRecord> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListPlaybookApprovalRequestsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn request_playbook_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<PlaybookApprovalRequestInput>,
) -> AppResult<Json<PlaybookApprovalRequestRecord>> {
    let actor = resolve_actor(&headers);
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;
    ensure_playbook_enabled(&playbook)?;
    if !playbook_requires_confirmation(&playbook) {
        return Err(AppError::Validation(
            "approval request is only required for high-risk playbooks".to_string(),
        ));
    }

    if payload.dry_run_id <= 0 {
        return Err(AppError::Validation(
            "dry_run_id must be a positive integer".to_string(),
        ));
    }

    let dry_run: Option<DryRunApprovalSourceRecord> = sqlx::query_as(
        "SELECT id, playbook_id, playbook_key, actor, mode, status, confirmation_required, expires_at
         FROM workflow_playbook_executions
         WHERE id = $1",
    )
    .bind(payload.dry_run_id)
    .fetch_optional(&state.db)
    .await?;
    let dry_run = dry_run.ok_or_else(|| {
        AppError::Validation(format!(
            "dry-run execution {} not found",
            payload.dry_run_id
        ))
    })?;

    if dry_run.mode != "dry_run" {
        return Err(AppError::Validation(format!(
            "execution {} is not a dry-run record",
            dry_run.id
        )));
    }
    if dry_run.playbook_id != playbook.id || dry_run.playbook_key != playbook.key {
        return Err(AppError::Validation(
            "dry-run execution belongs to another playbook".to_string(),
        ));
    }
    if !dry_run.actor.eq_ignore_ascii_case(actor.as_str()) {
        return Err(AppError::Forbidden(
            "only the dry-run requester can open approval request".to_string(),
        ));
    }
    if !dry_run.confirmation_required {
        return Err(AppError::Validation(
            "dry-run does not require high-risk approval".to_string(),
        ));
    }
    if !matches!(dry_run.status.as_str(), "planned" | "succeeded") {
        return Err(AppError::Validation(format!(
            "dry-run status '{}' cannot open approval request",
            dry_run.status
        )));
    }

    let now = Utc::now();
    let dry_run_expires_at = dry_run.expires_at.unwrap_or(now + Duration::minutes(APPROVAL_TTL_MINUTES));
    if dry_run_expires_at <= now {
        return Err(AppError::Validation(
            "dry-run has expired; create a new dry-run before requesting approval".to_string(),
        ));
    }

    if let Some(active) = load_active_playbook_approval_by_dry_run(&state.db, dry_run.id).await? {
        if active.expires_at <= now {
            expire_playbook_approval_request(&state.db, active.id).await?;
        } else {
            return Ok(Json(active));
        }
    }

    let note = trim_optional(payload.note, MAX_APPROVAL_NOTE_LEN);
    let expires_at = std::cmp::min(dry_run_expires_at, now + Duration::minutes(APPROVAL_TTL_MINUTES));
    let item: PlaybookApprovalRequestRecord = sqlx::query_as(
        "INSERT INTO workflow_playbook_approval_requests (
            dry_run_execution_id, playbook_id, playbook_key, requester, request_note, status, expires_at
         )
         VALUES ($1, $2, $3, $4, $5, 'pending', $6)
         RETURNING id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                   status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                   created_at, updated_at",
    )
    .bind(dry_run.id)
    .bind(playbook.id)
    .bind(playbook.key.clone())
    .bind(actor.clone())
    .bind(note.clone())
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.approval.request",
        "workflow_playbook_approval_request",
        Some(item.id.to_string()),
        "success",
        note,
        json!({
            "playbook_key": playbook.key,
            "dry_run_execution_id": dry_run.id,
            "expires_at": expires_at.to_rfc3339(),
        }),
    )
    .await;

    Ok(Json(item))
}

async fn approve_playbook_approval_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<PlaybookApprovalDecisionRequest>,
) -> AppResult<Json<PlaybookApprovalRequestRecord>> {
    let actor = resolve_actor(&headers);
    let note = trim_optional(payload.note, MAX_APPROVAL_NOTE_LEN);
    let current = load_playbook_approval_request(&state.db, id).await?;

    if current.requester.eq_ignore_ascii_case(actor.as_str()) {
        write_from_headers_best_effort(
            &state.db,
            &headers,
            "workflow.playbook.approval.approve",
            "workflow_playbook_approval_request",
            Some(id.to_string()),
            "failed",
            Some("self-approval is not allowed".to_string()),
            json!({
                "requester": current.requester,
                "actor": actor,
                "reason": "self_approval_forbidden",
            }),
        )
        .await;
        return Err(AppError::Forbidden(
            "self-approval is not allowed for high-risk playbook execution".to_string(),
        ));
    }
    if current.status != "pending" {
        return Err(AppError::Validation(format!(
            "approval request {} status '{}' cannot be approved",
            id, current.status
        )));
    }
    if current.expires_at <= Utc::now() {
        expire_playbook_approval_request(&state.db, id).await?;
        return Err(AppError::Validation(
            "approval request has expired and cannot be approved".to_string(),
        ));
    }

    let token = generate_approval_token();
    let updated: PlaybookApprovalRequestRecord = sqlx::query_as(
        "UPDATE workflow_playbook_approval_requests
         SET status = 'approved',
             approver = $2,
             approver_note = $3,
             approval_token = $4,
             approved_at = NOW(),
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                   status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                   created_at, updated_at",
    )
    .bind(id)
    .bind(actor.clone())
    .bind(note.clone())
    .bind(token)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.approval.approve",
        "workflow_playbook_approval_request",
        Some(id.to_string()),
        "success",
        note,
        json!({
            "requester": current.requester,
            "dry_run_execution_id": current.dry_run_execution_id,
        }),
    )
    .await;

    Ok(Json(updated))
}

async fn reject_playbook_approval_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<PlaybookApprovalDecisionRequest>,
) -> AppResult<Json<PlaybookApprovalRequestRecord>> {
    let actor = resolve_actor(&headers);
    let note = trim_optional(payload.note, MAX_APPROVAL_NOTE_LEN);
    let current = load_playbook_approval_request(&state.db, id).await?;
    if current.status != "pending" {
        return Err(AppError::Validation(format!(
            "approval request {} status '{}' cannot be rejected",
            id, current.status
        )));
    }
    if current.expires_at <= Utc::now() {
        expire_playbook_approval_request(&state.db, id).await?;
        return Err(AppError::Validation(
            "approval request has expired and cannot be rejected".to_string(),
        ));
    }

    let updated: PlaybookApprovalRequestRecord = sqlx::query_as(
        "UPDATE workflow_playbook_approval_requests
         SET status = 'rejected',
             approver = $2,
             approver_note = $3,
             approval_token = NULL,
             approved_at = NULL,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                   status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                   created_at, updated_at",
    )
    .bind(id)
    .bind(actor.clone())
    .bind(note.clone())
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.approval.reject",
        "workflow_playbook_approval_request",
        Some(id.to_string()),
        "success",
        note,
        json!({
            "requester": current.requester,
            "dry_run_execution_id": current.dry_run_execution_id,
        }),
    )
    .await;

    Ok(Json(updated))
}

async fn dry_run_playbook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<PlaybookRunRequest>,
) -> AppResult<Json<PlaybookDryRunResponse>> {
    let actor = resolve_actor(&headers);
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(&playbook);
    let confirmation_token = if confirmation_required {
        Some(generate_confirmation_token())
    } else {
        None
    };

    let now = Utc::now();
    let expires_at = if confirmation_required {
        Some(now + Duration::minutes(CONFIRMATION_TTL_MINUTES))
    } else {
        None
    };
    let status = if confirmation_required {
        "planned"
    } else {
        "succeeded"
    };
    let finished_at = if confirmation_required {
        None
    } else {
        Some(now)
    };

    let result = json!({
        "mode": "dry_run",
        "summary": dry_run_risk_summary_text(playbook.risk_level.as_str(), confirmation_required),
        "requires_confirmation": confirmation_required,
        "confirmation_token": confirmation_token,
        "next_actions": [
            {
                "label": "execute_playbook",
                "api": format!("/api/v1/workflow/playbooks/{}/execute", playbook.key),
                "method": "POST"
            }
        ]
    });

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "dry_run".to_string(),
            status: status.to_string(),
            confirmation_required,
            confirmation_token: confirmation_token.clone(),
            confirmation_verified: !confirmation_required,
            confirmed_at: None,
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id: None,
            expires_at,
            finished_at,
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.dry_run",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "playbook_key": playbook.key,
            "risk_level": playbook.risk_level,
            "confirmation_required": confirmation_required,
            "mode": "dry_run"
        }),
    )
    .await;

    write_alert_remediation_timeline_best_effort(&state.db, &execution, actor.as_str()).await;

    let confirmation = execution_confirmation_challenge(&execution);

    Ok(Json(PlaybookDryRunResponse {
        execution,
        risk_summary: DryRunRiskSummary {
            risk_level: playbook.risk_level.clone(),
            requires_confirmation: confirmation_required,
            ttl_minutes: CONFIRMATION_TTL_MINUTES,
            summary: dry_run_risk_summary_text(playbook.risk_level.as_str(), confirmation_required),
        },
        confirmation,
    }))
}

async fn execute_playbook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<PlaybookExecuteRequest>,
) -> AppResult<Json<PlaybookExecutionDetail>> {
    let actor = resolve_actor(&headers);
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let override_input = MaintenanceOverrideInput {
        reason: trim_optional(payload.maintenance_override_reason, MAX_OVERRIDE_REASON_LEN),
        confirmed: payload.maintenance_override_confirmed.unwrap_or(false),
    };
    enforce_playbook_execution_policy(&state, &headers, &playbook, actor.as_str(), override_input)
        .await?;

    let confirmation_required = playbook_requires_confirmation(&playbook);
    if confirmation_required {
        let approval_id = payload.approval_id.ok_or_else(|| {
            AppError::Validation(
                "approval_id is required for high-risk playbook execution".to_string(),
            )
        })?;
        let approval_token = required_trimmed_approval_token(payload.approval_token)?;
        verify_and_consume_approval(
            &state.db,
            approval_id,
            payload.dry_run_id,
            playbook.id,
            &actor,
            &approval_token,
        )
        .await?;

        let dry_run_id = payload.dry_run_id.ok_or_else(|| {
            AppError::Validation(
                "dry_run_id is required for high-risk playbook execution".to_string(),
            )
        })?;
        let confirmation_token = required_trimmed_token(payload.confirmation_token)?;

        verify_and_consume_confirmation(
            &state.db,
            dry_run_id,
            playbook.id,
            &actor,
            &confirmation_token,
        )
        .await?;
    }

    let now = Utc::now();
    let result = json!({
        "mode": "execute",
        "summary": "Playbook execution completed with audit trail.",
        "next_actions": [
            {
                "label": "open_workflow_page",
                "href": "#/workflow"
            },
            {
                "label": "open_alert_center",
                "href": "#/alerts"
            }
        ]
    });

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "execute".to_string(),
            status: "succeeded".to_string(),
            confirmation_required,
            confirmation_token: None,
            confirmation_verified: !confirmation_required,
            confirmed_at: if confirmation_required {
                Some(now)
            } else {
                None
            },
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id: None,
            expires_at: None,
            finished_at: Some(now),
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.execute",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "playbook_key": playbook.key,
            "risk_level": playbook.risk_level,
            "mode": "execute",
            "confirmation_required": confirmation_required
        }),
    )
    .await;

    write_alert_remediation_timeline_best_effort(&state.db, &execution, actor.as_str()).await;

    Ok(Json(execution))
}

async fn list_playbook_executions(
    State(state): State<AppState>,
    Query(query): Query<PlaybookExecutionListQuery>,
) -> AppResult<Json<ListPlaybookExecutionsResponse>> {
    let playbook_key = query.playbook_key.map(normalize_playbook_key).transpose()?;
    let mode = normalize_optional_mode(query.mode)?;
    let status = normalize_optional_execution_status(query.status)?;
    let actor = trim_optional(query.actor, MAX_ACTOR_LEN);
    let asset_ref = trim_optional(query.asset_ref, MAX_ASSET_REF_LEN);

    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM workflow_playbook_executions e WHERE 1=1");
    append_execution_filters(
        &mut count_builder,
        playbook_key.clone(),
        mode.clone(),
        status.clone(),
        actor.clone(),
        asset_ref.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            e.id,
            e.playbook_key,
            e.playbook_name,
            e.category,
            e.risk_level,
            e.actor,
            e.asset_ref,
            e.mode,
            e.status,
            e.confirmation_required,
            e.confirmation_verified,
            e.related_ticket_id,
            e.related_alert_id,
            e.replay_of_execution_id,
            e.created_at,
            e.finished_at
         FROM workflow_playbook_executions e
         WHERE 1=1",
    );

    append_execution_filters(
        &mut list_builder,
        playbook_key,
        mode,
        status,
        actor,
        asset_ref,
    );
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<PlaybookExecutionListItem> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListPlaybookExecutionsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_playbook_execution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<PlaybookExecutionDetail>> {
    let execution = load_playbook_execution_detail(&state.db, id).await?;
    Ok(Json(execution))
}

async fn replay_playbook_execution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<ReplayExecutionRequest>,
) -> AppResult<Json<ReplayExecutionResponse>> {
    let actor = resolve_actor(&headers);
    let source = load_playbook_execution_detail(&state.db, id).await?;

    let replay_mode = parse_replay_mode(payload.mode)?;
    let playbook = load_playbook_by_key(&state.db, &source.playbook_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let execution = match replay_mode {
        ReplayMode::DryRun => {
            let request = PlaybookRunRequest {
                params: Some(source.params.clone()),
                asset_ref: source.asset_ref.clone(),
                related_ticket_id: source.related_ticket_id,
                related_alert_id: source.related_alert_id,
            };
            run_replay_dry_run(
                &state,
                &headers,
                &playbook,
                request,
                Some(source.id),
                actor.clone(),
            )
            .await?
        }
        ReplayMode::Execute => {
            let request = PlaybookExecuteRequest {
                params: Some(source.params.clone()),
                asset_ref: source.asset_ref.clone(),
                dry_run_id: payload.dry_run_id,
                confirmation_token: payload.confirmation_token,
                approval_id: payload.approval_id,
                approval_token: payload.approval_token,
                maintenance_override_reason: payload.maintenance_override_reason,
                maintenance_override_confirmed: payload.maintenance_override_confirmed,
                related_ticket_id: source.related_ticket_id,
                related_alert_id: source.related_alert_id,
            };
            run_replay_execute(
                &state,
                &headers,
                &playbook,
                request,
                Some(source.id),
                actor.clone(),
            )
            .await?
        }
    };

    Ok(Json(ReplayExecutionResponse {
        mode: replay_mode.as_str().to_string(),
        source_execution_id: source.id,
        execution,
        note:
            "Replay request accepted. High-risk execute replay still requires dry-run confirmation."
                .to_string(),
    }))
}

async fn run_replay_dry_run(
    state: &AppState,
    headers: &HeaderMap,
    playbook: &PlaybookRecord,
    payload: PlaybookRunRequest,
    replay_of_execution_id: Option<i64>,
    actor: String,
) -> AppResult<PlaybookExecutionDetail> {
    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(playbook);
    let now = Utc::now();
    let expires_at = if confirmation_required {
        Some(now + Duration::minutes(CONFIRMATION_TTL_MINUTES))
    } else {
        None
    };
    let confirmation_token = if confirmation_required {
        Some(generate_confirmation_token())
    } else {
        None
    };

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "dry_run".to_string(),
            status: if confirmation_required {
                "planned".to_string()
            } else {
                "succeeded".to_string()
            },
            confirmation_required,
            confirmation_token: confirmation_token.clone(),
            confirmation_verified: !confirmation_required,
            confirmed_at: None,
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result: json!({
                "mode": "dry_run",
                "summary": "Replay dry-run generated from historical execution.",
                "confirmation_token": confirmation_token,
            }),
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at: if confirmation_required {
                None
            } else {
                Some(now)
            },
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        headers,
        "workflow.playbook.replay",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "mode": "dry_run",
            "playbook_key": playbook.key,
            "replay_of_execution_id": replay_of_execution_id,
        }),
    )
    .await;

    write_alert_remediation_timeline_best_effort(&state.db, &execution, actor.as_str()).await;

    Ok(execution)
}

async fn run_replay_execute(
    state: &AppState,
    headers: &HeaderMap,
    playbook: &PlaybookRecord,
    payload: PlaybookExecuteRequest,
    replay_of_execution_id: Option<i64>,
    actor: String,
) -> AppResult<PlaybookExecutionDetail> {
    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let override_input = MaintenanceOverrideInput {
        reason: trim_optional(payload.maintenance_override_reason, MAX_OVERRIDE_REASON_LEN),
        confirmed: payload.maintenance_override_confirmed.unwrap_or(false),
    };
    enforce_playbook_execution_policy(state, headers, playbook, actor.as_str(), override_input)
        .await?;

    let confirmation_required = playbook_requires_confirmation(playbook);
    if confirmation_required {
        let approval_id = payload.approval_id.ok_or_else(|| {
            AppError::Validation(
                "approval_id is required for high-risk playbook replay execution".to_string(),
            )
        })?;
        let approval_token = required_trimmed_approval_token(payload.approval_token)?;
        verify_and_consume_approval(
            &state.db,
            approval_id,
            payload.dry_run_id,
            playbook.id,
            &actor,
            &approval_token,
        )
        .await?;

        let dry_run_id = payload.dry_run_id.ok_or_else(|| {
            AppError::Validation(
                "dry_run_id is required for high-risk playbook replay execution".to_string(),
            )
        })?;
        let confirmation_token = required_trimmed_token(payload.confirmation_token)?;

        verify_and_consume_confirmation(
            &state.db,
            dry_run_id,
            playbook.id,
            &actor,
            &confirmation_token,
        )
        .await?;
    }

    let now = Utc::now();
    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "execute".to_string(),
            status: "succeeded".to_string(),
            confirmation_required,
            confirmation_token: None,
            confirmation_verified: !confirmation_required,
            confirmed_at: if confirmation_required {
                Some(now)
            } else {
                None
            },
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result: json!({
                "mode": "execute",
                "summary": "Replay execute completed with the same validated parameters.",
            }),
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at: None,
            finished_at: Some(now),
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        headers,
        "workflow.playbook.replay",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "mode": "execute",
            "playbook_key": playbook.key,
            "replay_of_execution_id": replay_of_execution_id,
        }),
    )
    .await;

    write_alert_remediation_timeline_best_effort(&state.db, &execution, actor.as_str()).await;

    Ok(execution)
}

async fn load_playbook_by_key(db: &sqlx::PgPool, key: &str) -> AppResult<PlaybookRecord> {
    let item: Option<PlaybookRecord> = sqlx::query_as(
        "SELECT id,
                playbook_key AS key,
                name,
                description,
                category,
                risk_level,
                requires_confirmation,
                parameter_schema AS params,
                execution_plan,
                rbac_hint,
                is_enabled,
                is_system,
                created_at,
                updated_at
         FROM workflow_playbooks
         WHERE playbook_key = $1",
    )
    .bind(key)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("playbook '{key}' not found")))
}

async fn load_playbook_execution_detail(
    db: &sqlx::PgPool,
    id: i64,
) -> AppResult<PlaybookExecutionDetail> {
    let item: Option<PlaybookExecutionDetail> = sqlx::query_as(
        "SELECT
            id,
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_verified,
            confirmed_at,
            params_json AS params,
            planned_steps,
            result_json AS result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at,
            created_at,
            updated_at
         FROM workflow_playbook_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("playbook execution {id} not found")))
}

async fn load_playbook_approval_request(
    db: &sqlx::PgPool,
    id: i64,
) -> AppResult<PlaybookApprovalRequestRecord> {
    let item: Option<PlaybookApprovalRequestRecord> = sqlx::query_as(
        "SELECT id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                created_at, updated_at
         FROM workflow_playbook_approval_requests
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("approval request {id} not found")))
}

async fn load_active_playbook_approval_by_dry_run(
    db: &sqlx::PgPool,
    dry_run_id: i64,
) -> AppResult<Option<PlaybookApprovalRequestRecord>> {
    sqlx::query_as(
        "SELECT id, dry_run_execution_id, playbook_id, playbook_key, requester, request_note,
                status, approver, approver_note, approval_token, approved_at, expires_at, used_at,
                created_at, updated_at
         FROM workflow_playbook_approval_requests
         WHERE dry_run_execution_id = $1
           AND status IN ('pending', 'approved')
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(dry_run_id)
    .fetch_optional(db)
    .await
    .map_err(AppError::from)
}

async fn expire_playbook_approval_request(db: &sqlx::PgPool, id: i64) -> AppResult<()> {
    sqlx::query(
        "UPDATE workflow_playbook_approval_requests
         SET status = 'expired',
             updated_at = NOW()
         WHERE id = $1
           AND status IN ('pending', 'approved')",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

fn build_policy_response(
    policy: &PlaybookExecutionPolicy,
    now_utc: DateTime<Utc>,
) -> PlaybookExecutionPolicyResponse {
    let runtime = evaluate_policy_runtime(policy, now_utc);
    PlaybookExecutionPolicyResponse {
        policy: PlaybookExecutionPolicyView {
            policy_key: policy.policy_key.clone(),
            timezone_name: policy.timezone_name.clone(),
            maintenance_windows: policy.maintenance_windows.clone(),
            change_freeze_enabled: policy.change_freeze_enabled,
            override_requires_reason: policy.override_requires_reason,
            updated_by: policy.updated_by.clone(),
            updated_at: policy.updated_at,
        },
        runtime,
    }
}

async fn load_or_init_playbook_execution_policy(
    db: &sqlx::PgPool,
) -> AppResult<PlaybookExecutionPolicy> {
    let existing: Option<PlaybookExecutionPolicyRow> = sqlx::query_as(
        "SELECT policy_key,
                timezone_name,
                maintenance_windows,
                change_freeze_enabled,
                override_requires_reason,
                updated_by,
                updated_at
         FROM workflow_playbook_execution_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(PLAYBOOK_POLICY_DEFAULT_KEY)
    .fetch_optional(db)
    .await?;

    if let Some(row) = existing {
        return parse_playbook_execution_policy_row(row, None);
    }

    let default_windows = default_maintenance_windows();
    let inserted: PlaybookExecutionPolicyRow = sqlx::query_as(
        "INSERT INTO workflow_playbook_execution_policies (
            policy_key,
            timezone_name,
            maintenance_windows,
            change_freeze_enabled,
            override_requires_reason,
            updated_by
         )
         VALUES ($1, 'UTC', $2, FALSE, TRUE, 'system')
         RETURNING policy_key, timezone_name, maintenance_windows, change_freeze_enabled,
                   override_requires_reason, updated_by, updated_at",
    )
    .bind(PLAYBOOK_POLICY_DEFAULT_KEY)
    .bind(serde_json::to_value(&default_windows).map_err(|err| {
        AppError::Validation(format!(
            "failed to serialize default maintenance windows: {err}"
        ))
    })?)
    .fetch_one(db)
    .await?;

    parse_playbook_execution_policy_row(inserted, None)
}

fn parse_playbook_execution_policy_row(
    row: PlaybookExecutionPolicyRow,
    timezone_override: Option<Tz>,
) -> AppResult<PlaybookExecutionPolicy> {
    let timezone_name = normalize_timezone_name(row.timezone_name)?;
    let timezone = if let Some(value) = timezone_override {
        value
    } else {
        parse_timezone(timezone_name.as_str())?
    };
    let maintenance_windows = parse_maintenance_windows_value(row.maintenance_windows)?;

    Ok(PlaybookExecutionPolicy {
        policy_key: row.policy_key,
        timezone_name,
        timezone,
        maintenance_windows,
        change_freeze_enabled: row.change_freeze_enabled,
        override_requires_reason: row.override_requires_reason,
        updated_by: row.updated_by,
        updated_at: row.updated_at,
    })
}

fn default_maintenance_windows() -> Vec<PlaybookMaintenanceWindow> {
    (1u8..=7u8)
        .map(|day| PlaybookMaintenanceWindow {
            day_of_week: day,
            start: "00:00".to_string(),
            end: "23:59".to_string(),
            label: Some("full-day".to_string()),
        })
        .collect()
}

fn parse_timezone(value: &str) -> AppResult<Tz> {
    value.parse::<Tz>().map_err(|_| {
        AppError::Validation(format!(
            "timezone_name '{value}' is invalid (example: UTC, Asia/Shanghai, America/New_York)"
        ))
    })
}

fn normalize_timezone_name(value: String) -> AppResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "timezone_name cannot be empty".to_string(),
        ));
    }
    if normalized.chars().count() > MAX_POLICY_TIMEZONE_LEN {
        return Err(AppError::Validation(format!(
            "timezone_name length must be <= {MAX_POLICY_TIMEZONE_LEN}"
        )));
    }
    Ok(normalized.to_string())
}

fn parse_maintenance_windows_value(value: Value) -> AppResult<Vec<PlaybookMaintenanceWindow>> {
    let windows: Vec<PlaybookMaintenanceWindow> = serde_json::from_value(value).map_err(|err| {
        AppError::Validation(format!("maintenance_windows is invalid JSON structure: {err}"))
    })?;
    normalize_maintenance_windows(windows)
}

fn normalize_maintenance_windows(
    windows: Vec<PlaybookMaintenanceWindow>,
) -> AppResult<Vec<PlaybookMaintenanceWindow>> {
    if windows.is_empty() {
        return Err(AppError::Validation(
            "maintenance_windows must include at least one window".to_string(),
        ));
    }
    if windows.len() > 84 {
        return Err(AppError::Validation(
            "maintenance_windows count must be <= 84".to_string(),
        ));
    }

    let mut normalized = Vec::with_capacity(windows.len());
    for window in windows {
        if !(1..=7).contains(&window.day_of_week) {
            return Err(AppError::Validation(
                "maintenance window day_of_week must be in [1,7] where Monday=1".to_string(),
            ));
        }

        let start = normalize_hhmm(window.start.as_str(), "start")?;
        let end = normalize_hhmm(window.end.as_str(), "end")?;
        let start_time = parse_hhmm(start.as_str()).map_err(|_| {
            AppError::Validation("maintenance window start must use HH:MM format".to_string())
        })?;
        let end_time = parse_hhmm(end.as_str()).map_err(|_| {
            AppError::Validation("maintenance window end must use HH:MM format".to_string())
        })?;
        if start_time >= end_time {
            return Err(AppError::Validation(format!(
                "maintenance window start '{}' must be earlier than end '{}'",
                start, end
            )));
        }

        let label = window
            .label
            .and_then(|value| trim_optional(Some(value), MAX_WINDOW_LABEL_LEN));
        normalized.push(PlaybookMaintenanceWindow {
            day_of_week: window.day_of_week,
            start,
            end,
            label,
        });
    }

    normalized.sort_by(|left, right| {
        left.day_of_week
            .cmp(&right.day_of_week)
            .then_with(|| left.start.cmp(&right.start))
            .then_with(|| left.end.cmp(&right.end))
    });
    Ok(normalized)
}

fn normalize_hhmm(value: &str, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let parsed = parse_hhmm(trimmed).map_err(|_| {
        AppError::Validation(format!(
            "maintenance window {field} must use HH:MM 24h format"
        ))
    })?;
    Ok(format!(
        "{:02}:{:02}",
        parsed.hour(),
        parsed.minute()
    ))
}

fn parse_hhmm(value: &str) -> Result<NaiveTime, chrono::ParseError> {
    NaiveTime::parse_from_str(value, "%H:%M")
}

fn evaluate_policy_runtime(
    policy: &PlaybookExecutionPolicy,
    now_utc: DateTime<Utc>,
) -> PlaybookExecutionPolicyRuntime {
    let now_local = now_utc.with_timezone(&policy.timezone);
    let in_maintenance_window = is_in_maintenance_window(policy, now_utc);
    let next_allowed_at = next_allowed_at_utc(policy, now_utc).map(|value| value.to_rfc3339());

    let blocked_reason = if policy.change_freeze_enabled {
        Some("change-freeze is enabled".to_string())
    } else if !in_maintenance_window {
        Some("outside configured maintenance windows".to_string())
    } else {
        None
    };

    PlaybookExecutionPolicyRuntime {
        timezone_now: now_local.to_rfc3339(),
        in_maintenance_window,
        next_allowed_at,
        blocked_reason,
    }
}

fn is_in_maintenance_window(policy: &PlaybookExecutionPolicy, now_utc: DateTime<Utc>) -> bool {
    if policy.maintenance_windows.is_empty() {
        return false;
    }

    let now_local = now_utc.with_timezone(&policy.timezone);
    let day_of_week = now_local.weekday().number_from_monday() as u8;
    let current_time = now_local.time();

    policy.maintenance_windows.iter().any(|window| {
        if window.day_of_week != day_of_week {
            return false;
        }

        let start = match parse_hhmm(window.start.as_str()) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let end = match parse_hhmm(window.end.as_str()) {
            Ok(value) => value,
            Err(_) => return false,
        };
        current_time >= start && current_time < end
    })
}

fn next_allowed_at_utc(policy: &PlaybookExecutionPolicy, now_utc: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if policy.maintenance_windows.is_empty() {
        return None;
    }

    let now_local = now_utc.with_timezone(&policy.timezone);
    let now_naive = now_local.naive_local();

    for day_offset in 0i64..14i64 {
        let candidate_date = now_naive.date() + Duration::days(day_offset);
        let weekday = candidate_date.weekday().number_from_monday() as u8;

        for window in policy
            .maintenance_windows
            .iter()
            .filter(|item| item.day_of_week == weekday)
        {
            let start = match parse_hhmm(window.start.as_str()) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let candidate_naive = candidate_date.and_time(start);
            if day_offset == 0 && candidate_naive <= now_naive {
                continue;
            }

            let candidate_local = match policy.timezone.from_local_datetime(&candidate_naive) {
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(earliest, _) => earliest,
                LocalResult::None => continue,
            };
            return Some(candidate_local.with_timezone(&Utc));
        }
    }

    None
}

async fn enforce_playbook_execution_policy(
    state: &AppState,
    headers: &HeaderMap,
    playbook: &PlaybookRecord,
    actor: &str,
    override_input: MaintenanceOverrideInput,
) -> AppResult<()> {
    if !playbook_requires_confirmation(playbook) {
        return Ok(());
    }

    let policy = load_or_init_playbook_execution_policy(&state.db).await?;
    let runtime = evaluate_policy_runtime(&policy, Utc::now());

    if runtime.blocked_reason.is_none() {
        return Ok(());
    }

    if override_input.confirmed {
        let reason = override_input
            .reason
            .clone()
            .filter(|value| !value.trim().is_empty());
        if policy.override_requires_reason && reason.is_none() {
            return Err(AppError::Validation(
                "maintenance_override_reason is required when override is requested".to_string(),
            ));
        }

        write_from_headers_best_effort(
            &state.db,
            headers,
            "workflow.playbook.policy.override",
            "workflow_playbook",
            Some(playbook.key.clone()),
            "success",
            reason.clone(),
            json!({
                "playbook_key": playbook.key,
                "actor": actor,
                "change_freeze_enabled": policy.change_freeze_enabled,
                "override_requires_reason": policy.override_requires_reason,
                "blocked_reason": runtime.blocked_reason,
                "next_allowed_at": runtime.next_allowed_at,
            }),
        )
        .await;

        return Ok(());
    }

    let next_hint = runtime
        .next_allowed_at
        .map(|value| format!(" next_allowed_at={value}"))
        .unwrap_or_default();
    let blocked_reason = runtime
        .blocked_reason
        .unwrap_or_else(|| "blocked by execution policy".to_string());
    Err(AppError::Validation(format!(
        "playbook execution blocked by policy: {blocked_reason}.{next_hint} To override, set maintenance_override_confirmed=true and provide maintenance_override_reason."
    )))
}

fn append_playbook_filters(
    builder: &mut QueryBuilder<Postgres>,
    category: Option<String>,
    risk_level: Option<String>,
    is_enabled: Option<bool>,
    query_text: Option<String>,
) {
    if let Some(category) = category {
        builder.push(" AND p.category = ").push_bind(category);
    }
    if let Some(risk_level) = risk_level {
        builder.push(" AND p.risk_level = ").push_bind(risk_level);
    }
    if let Some(is_enabled) = is_enabled {
        builder.push(" AND p.is_enabled = ").push_bind(is_enabled);
    }
    if let Some(query_text) = query_text {
        let like = format!("%{query_text}%");
        builder.push(" AND (");
        builder
            .push("p.playbook_key ILIKE ")
            .push_bind(like.clone());
        builder.push(" OR p.name ILIKE ").push_bind(like.clone());
        builder
            .push(" OR COALESCE(p.description, '') ILIKE ")
            .push_bind(like);
        builder.push(")");
    }
}

fn append_execution_filters(
    builder: &mut QueryBuilder<Postgres>,
    playbook_key: Option<String>,
    mode: Option<String>,
    status: Option<String>,
    actor: Option<String>,
    asset_ref: Option<String>,
) {
    if let Some(playbook_key) = playbook_key {
        builder
            .push(" AND e.playbook_key = ")
            .push_bind(playbook_key);
    }
    if let Some(mode) = mode {
        builder.push(" AND e.mode = ").push_bind(mode);
    }
    if let Some(status) = status {
        builder.push(" AND e.status = ").push_bind(status);
    }
    if let Some(actor) = actor {
        builder
            .push(" AND e.actor ILIKE ")
            .push_bind(format!("%{actor}%"));
    }
    if let Some(asset_ref) = asset_ref {
        builder
            .push(" AND e.asset_ref ILIKE ")
            .push_bind(format!("%{asset_ref}%"));
    }
}

fn append_approval_filters(
    builder: &mut QueryBuilder<Postgres>,
    playbook_key: Option<String>,
    status: Option<String>,
    requester: Option<String>,
    approver: Option<String>,
) {
    if let Some(playbook_key) = playbook_key {
        builder
            .push(" AND a.playbook_key = ")
            .push_bind(playbook_key);
    }
    if let Some(status) = status {
        builder.push(" AND a.status = ").push_bind(status);
    }
    if let Some(requester) = requester {
        builder
            .push(" AND a.requester ILIKE ")
            .push_bind(format!("%{requester}%"));
    }
    if let Some(approver) = approver {
        builder
            .push(" AND COALESCE(a.approver, '') ILIKE ")
            .push_bind(format!("%{approver}%"));
    }
}

async fn verify_and_consume_confirmation(
    db: &sqlx::PgPool,
    dry_run_id: i64,
    playbook_id: i64,
    actor: &str,
    confirmation_token: &str,
) -> AppResult<()> {
    if dry_run_id <= 0 {
        return Err(AppError::Validation(
            "dry_run_id must be a positive integer".to_string(),
        ));
    }

    let record: Option<DryRunConfirmationRecord> = sqlx::query_as(
        "SELECT
            id,
            playbook_id,
            actor,
            confirmation_token,
            confirmation_required,
            expires_at,
            mode,
            status
         FROM workflow_playbook_executions
         WHERE id = $1",
    )
    .bind(dry_run_id)
    .fetch_optional(db)
    .await?;

    let record = record
        .ok_or_else(|| AppError::Validation(format!("dry-run execution {dry_run_id} not found")))?;

    let now = Utc::now();
    match validate_confirmation_transition(&record, playbook_id, actor, confirmation_token, now)? {
        ConfirmationDecision::Allow => {}
        ConfirmationDecision::Expired => {
            sqlx::query(
                "UPDATE workflow_playbook_executions
                 SET status = 'expired', updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(record.id)
            .execute(db)
            .await?;
            return Err(AppError::Validation(
                "dry-run confirmation has expired; create a new dry-run".to_string(),
            ));
        }
    }

    sqlx::query(
        "UPDATE workflow_playbook_executions
         SET confirmation_verified = TRUE,
             confirmed_at = NOW(),
             status = 'succeeded',
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(record.id)
    .execute(db)
    .await?;

    Ok(())
}

async fn verify_and_consume_approval(
    db: &sqlx::PgPool,
    approval_id: i64,
    dry_run_id: Option<i64>,
    playbook_id: i64,
    actor: &str,
    approval_token: &str,
) -> AppResult<()> {
    if approval_id <= 0 {
        return Err(AppError::Validation(
            "approval_id must be a positive integer".to_string(),
        ));
    }

    let record: Option<ApprovalConsumeRecord> = sqlx::query_as(
        "SELECT id, dry_run_execution_id, playbook_id, requester, status, approver, approval_token, expires_at
         FROM workflow_playbook_approval_requests
         WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_optional(db)
    .await?;
    let record = record.ok_or_else(|| {
        AppError::Validation(format!("approval request {} not found", approval_id))
    })?;

    if record.playbook_id != playbook_id {
        return Err(AppError::Validation(
            "approval request belongs to another playbook".to_string(),
        ));
    }
    if let Some(dry_run_id) = dry_run_id {
        if dry_run_id != record.dry_run_execution_id {
            return Err(AppError::Validation(
                "approval request is not bound to provided dry_run_id".to_string(),
            ));
        }
    }
    if !record.requester.eq_ignore_ascii_case(actor) {
        return Err(AppError::Forbidden(
            "only the original requester can consume approval token".to_string(),
        ));
    }
    if record
        .approver
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(actor))
    {
        return Err(AppError::Forbidden(
            "self-approval cannot be used for execution".to_string(),
        ));
    }
    if record.status == "used" {
        return Err(AppError::Validation(
            "approval request token has already been used".to_string(),
        ));
    }
    if record.status == "rejected" {
        return Err(AppError::Validation(
            "approval request was rejected and cannot be consumed".to_string(),
        ));
    }
    if record.status == "expired" {
        return Err(AppError::Validation(
            "approval request has expired".to_string(),
        ));
    }
    if record.status != "approved" {
        return Err(AppError::Validation(format!(
            "approval request status '{}' is not consumable",
            record.status
        )));
    }
    if record.expires_at <= Utc::now() {
        expire_playbook_approval_request(db, record.id).await?;
        return Err(AppError::Validation(
            "approval request has expired".to_string(),
        ));
    }
    let expected_token = record
        .approval_token
        .as_deref()
        .ok_or_else(|| AppError::Validation("approval token is missing".to_string()))?;
    if expected_token != approval_token {
        return Err(AppError::Validation(
            "approval_token does not match approval request".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE workflow_playbook_approval_requests
         SET status = 'used',
             used_at = NOW(),
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(record.id)
    .execute(db)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmationDecision {
    Allow,
    Expired,
}

fn validate_confirmation_transition(
    record: &DryRunConfirmationRecord,
    playbook_id: i64,
    actor: &str,
    confirmation_token: &str,
    now: DateTime<Utc>,
) -> AppResult<ConfirmationDecision> {
    if record.mode != "dry_run" {
        return Err(AppError::Validation(format!(
            "execution {} is not a dry-run record",
            record.id
        )));
    }
    if record.playbook_id != playbook_id {
        return Err(AppError::Validation(
            "dry-run execution belongs to another playbook".to_string(),
        ));
    }
    if !record.actor.eq_ignore_ascii_case(actor) {
        return Err(AppError::Forbidden(
            "dry-run confirmation can only be consumed by the same actor".to_string(),
        ));
    }
    if !record.confirmation_required {
        return Err(AppError::Validation(
            "dry-run confirmation is not required for this playbook".to_string(),
        ));
    }
    if !matches!(record.status.as_str(), "planned" | "succeeded") {
        return Err(AppError::Validation(format!(
            "dry-run execution status '{}' cannot be used for confirmation",
            record.status
        )));
    }
    if let Some(expires_at) = record.expires_at {
        if expires_at < now {
            return Ok(ConfirmationDecision::Expired);
        }
    }
    let expected_token = record
        .confirmation_token
        .as_deref()
        .ok_or_else(|| AppError::Validation("dry-run confirmation token is missing".to_string()))?;
    if expected_token != confirmation_token {
        return Err(AppError::Validation(
            "confirmation_token does not match dry-run challenge".to_string(),
        ));
    }

    Ok(ConfirmationDecision::Allow)
}

struct PlaybookExecutionInsertInput {
    playbook_id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_token: Option<String>,
    confirmation_verified: bool,
    confirmed_at: Option<DateTime<Utc>>,
    params: Value,
    planned_steps: Value,
    result: Value,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    expires_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

async fn insert_playbook_execution(
    db: &sqlx::PgPool,
    input: PlaybookExecutionInsertInput,
) -> AppResult<PlaybookExecutionDetail> {
    let item: PlaybookExecutionDetail = sqlx::query_as(
        "INSERT INTO workflow_playbook_executions (
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_token,
            confirmation_verified,
            confirmed_at,
            params_json,
            planned_steps,
            result_json,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at
         )
         VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
            $11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21
         )
         RETURNING
            id,
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_verified,
            confirmed_at,
            params_json AS params,
            planned_steps,
            result_json AS result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at,
            created_at,
            updated_at",
    )
    .bind(input.playbook_id)
    .bind(input.playbook_key)
    .bind(input.playbook_name)
    .bind(input.category)
    .bind(input.risk_level)
    .bind(input.actor)
    .bind(input.asset_ref)
    .bind(input.mode)
    .bind(input.status)
    .bind(input.confirmation_required)
    .bind(input.confirmation_token)
    .bind(input.confirmation_verified)
    .bind(input.confirmed_at)
    .bind(input.params)
    .bind(input.planned_steps)
    .bind(input.result)
    .bind(input.related_ticket_id)
    .bind(input.related_alert_id)
    .bind(input.replay_of_execution_id)
    .bind(input.expires_at)
    .bind(input.finished_at)
    .fetch_one(db)
    .await?;

    Ok(item)
}

async fn write_alert_remediation_timeline_best_effort(
    db: &sqlx::PgPool,
    execution: &PlaybookExecutionDetail,
    actor: &str,
) {
    let Some(alert_id) = execution.related_alert_id else {
        return;
    };

    let event_type = if execution.mode == "dry_run" {
        "remediation_dry_run"
    } else {
        "remediation_executed"
    };
    let message = if execution.mode == "dry_run" {
        Some(format!(
            "Remediation dry-run prepared with playbook '{}' (execution #{})",
            execution.playbook_key, execution.id
        ))
    } else {
        Some(format!(
            "Remediation execution finished with playbook '{}' status='{}' (execution #{})",
            execution.playbook_key, execution.status, execution.id
        ))
    };

    if let Err(err) = append_alert_remediation_timeline(
        db,
        alert_id,
        event_type,
        actor,
        message,
        json!({
            "playbook_execution_id": execution.id,
            "playbook_key": execution.playbook_key,
            "playbook_name": execution.playbook_name,
            "mode": execution.mode,
            "status": execution.status,
            "risk_level": execution.risk_level,
            "confirmation_required": execution.confirmation_required,
            "confirmation_verified": execution.confirmation_verified,
            "replay_of_execution_id": execution.replay_of_execution_id,
        }),
    )
    .await
    {
        warn!(
            error = %err,
            alert_id,
            playbook_execution_id = execution.id,
            "failed to append alert remediation timeline event"
        );
    }
}

fn parse_parameter_schema(schema: Value) -> AppResult<PlaybookParameterSchema> {
    let schema: PlaybookParameterSchema = serde_json::from_value(schema).map_err(|err| {
        AppError::Validation(format!("playbook parameter schema is invalid: {err}"))
    })?;

    if schema.fields.len() > MAX_PARAM_FIELDS {
        return Err(AppError::Validation(format!(
            "playbook parameter schema fields must be <= {MAX_PARAM_FIELDS}"
        )));
    }

    let mut seen = BTreeSet::new();
    for field in &schema.fields {
        let key = normalize_param_field_key(field.key.clone())?;
        if !seen.insert(key.clone()) {
            return Err(AppError::Validation(format!(
                "playbook parameter field '{key}' is duplicated"
            )));
        }

        let field_type = normalize_field_type(field.field_type.clone())?;
        if field_type == "enum" {
            let Some(options) = field.options.as_ref() else {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' type enum requires options"
                )));
            };
            if options.is_empty() || options.len() > MAX_PARAM_FIELD_OPTIONS {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' enum options must be between 1 and {MAX_PARAM_FIELD_OPTIONS}"
                )));
            }
        }

        if let (Some(min), Some(max)) = (field.min, field.max) {
            if min > max {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' has min > max"
                )));
            }
        }
    }

    Ok(schema)
}

fn parse_execution_plan_steps(plan: Value) -> AppResult<Vec<String>> {
    let plan: PlaybookExecutionPlan = serde_json::from_value(plan).map_err(|err| {
        AppError::Validation(format!("playbook execution plan is invalid: {err}"))
    })?;

    if plan.steps.is_empty() {
        return Err(AppError::Validation(
            "playbook execution plan must include at least one step".to_string(),
        ));
    }
    if plan.steps.len() > MAX_PLAN_STEPS {
        return Err(AppError::Validation(format!(
            "playbook execution plan steps must be <= {MAX_PLAN_STEPS}"
        )));
    }

    let mut normalized_steps = Vec::with_capacity(plan.steps.len());
    for raw_step in plan.steps {
        let step = raw_step.trim();
        if step.is_empty() {
            return Err(AppError::Validation(
                "playbook execution plan contains an empty step".to_string(),
            ));
        }
        if step.chars().count() > MAX_STEP_TEXT_LEN {
            return Err(AppError::Validation(format!(
                "playbook execution step length must be <= {MAX_STEP_TEXT_LEN}"
            )));
        }
        normalized_steps.push(step.to_string());
    }

    Ok(normalized_steps)
}

fn normalize_playbook_params(
    params: Option<Value>,
    schema: &PlaybookParameterSchema,
) -> AppResult<Value> {
    let raw = params.unwrap_or_else(|| json!({}));
    let raw_object = raw
        .as_object()
        .ok_or_else(|| AppError::Validation("playbook params must be a JSON object".to_string()))?;

    let mut allowed = BTreeSet::new();
    let mut output = JsonMap::new();

    for field in &schema.fields {
        let key = normalize_param_field_key(field.key.clone())?;
        allowed.insert(key.clone());

        let input_value = raw_object
            .get(&key)
            .cloned()
            .or_else(|| field.default.clone());

        if input_value.is_none() {
            if field.required {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' is required"
                )));
            }
            continue;
        }

        let value = input_value.expect("checked is_some");
        let normalized = validate_field_value(&key, field, value)?;
        output.insert(key, normalized);
    }

    let unknown_keys: Vec<String> = raw_object
        .keys()
        .filter(|key| !allowed.contains(*key))
        .map(|key| key.to_string())
        .collect();

    if !unknown_keys.is_empty() {
        return Err(AppError::Validation(format!(
            "unknown playbook params: {}",
            unknown_keys.join(", ")
        )));
    }

    Ok(Value::Object(output))
}

fn validate_field_value(
    key: &str,
    field: &PlaybookParameterField,
    value: Value,
) -> AppResult<Value> {
    let field_type = normalize_field_type(field.field_type.clone())?;

    match field_type.as_str() {
        "string" => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be a string"))
            })?;
            let text = text.trim();
            if field.required && text.is_empty() {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' cannot be empty"
                )));
            }
            if let Some(max_length) = field.max_length {
                if text.chars().count() > max_length {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' length must be <= {max_length}"
                    )));
                }
            }
            Ok(Value::String(text.to_string()))
        }
        "integer" => {
            let number = value.as_i64().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be an integer"))
            })?;
            if let Some(min) = field.min {
                if (number as f64) < min {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be >= {min}"
                    )));
                }
            }
            if let Some(max) = field.max {
                if (number as f64) > max {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be <= {max}"
                    )));
                }
            }
            Ok(Value::Number(number.into()))
        }
        "number" => {
            let number = value.as_f64().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be numeric"))
            })?;
            if let Some(min) = field.min {
                if number < min {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be >= {min}"
                    )));
                }
            }
            if let Some(max) = field.max {
                if number > max {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be <= {max}"
                    )));
                }
            }
            Ok(json!(number))
        }
        "boolean" => {
            let bool_value = value.as_bool().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be boolean"))
            })?;
            Ok(Value::Bool(bool_value))
        }
        "enum" => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "playbook param '{key}' enum value must be a string"
                ))
            })?;
            let normalized = text.trim();
            let options = field.options.as_ref().ok_or_else(|| {
                AppError::Validation(format!(
                    "playbook parameter schema for '{key}' is missing enum options"
                ))
            })?;
            let options_set = options
                .iter()
                .map(|item| item.trim().to_string())
                .collect::<BTreeSet<_>>();
            if !options_set.contains(normalized) {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' must be one of: {}",
                    options_set.into_iter().collect::<Vec<_>>().join(", ")
                )));
            }
            Ok(Value::String(normalized.to_string()))
        }
        _ => Err(AppError::Validation(format!(
            "playbook parameter field '{key}' has unsupported type '{field_type}'"
        ))),
    }
}

fn normalize_param_field_key(key: String) -> AppResult<String> {
    let key = key.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(AppError::Validation(
            "playbook parameter field key cannot be empty".to_string(),
        ));
    }
    if key.len() > MAX_PARAM_FIELD_KEY_LEN {
        return Err(AppError::Validation(format!(
            "playbook parameter field key length must be <= {MAX_PARAM_FIELD_KEY_LEN}"
        )));
    }
    if !key
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "playbook parameter field key can only include [a-z0-9_-]".to_string(),
        ));
    }
    Ok(key)
}

fn normalize_field_type(field_type: String) -> AppResult<String> {
    let normalized = field_type.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "playbook parameter field type is required".to_string(),
        ));
    }
    if normalized.len() > MAX_PARAM_FIELD_TYPE_LEN {
        return Err(AppError::Validation(format!(
            "playbook parameter field type length must be <= {MAX_PARAM_FIELD_TYPE_LEN}"
        )));
    }
    if !matches!(
        normalized.as_str(),
        "string" | "integer" | "number" | "boolean" | "enum"
    ) {
        return Err(AppError::Validation(format!(
            "unsupported parameter field type '{normalized}'"
        )));
    }
    Ok(normalized)
}

fn normalize_optional_risk_level(value: Option<String>) -> AppResult<Option<String>> {
    value.map(normalize_risk_level).transpose()
}

fn normalize_risk_level(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => Ok(normalized),
        _ => Err(AppError::Validation(
            "risk_level must be one of: low, medium, high, critical".to_string(),
        )),
    }
}

fn normalize_playbook_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("playbook key is required".to_string()));
    }
    if normalized.len() > MAX_PLAYBOOK_KEY_LEN {
        return Err(AppError::Validation(format!(
            "playbook key length must be <= {MAX_PLAYBOOK_KEY_LEN}"
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "playbook key can only include lowercase letters, numbers, '_' and '-'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_optional_mode(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "dry_run" | "execute" => Ok(normalized),
                _ => Err(AppError::Validation(
                    "mode must be one of: dry_run, execute".to_string(),
                )),
            }
        })
        .transpose()
}

fn normalize_optional_execution_status(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "planned" | "succeeded" | "failed" | "blocked" | "expired" => Ok(normalized),
                _ => Err(AppError::Validation(
                    "status must be one of: planned, succeeded, failed, blocked, expired"
                        .to_string(),
                )),
            }
        })
        .transpose()
}

fn normalize_optional_approval_status(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "pending" | "approved" | "rejected" | "expired" | "used" => Ok(normalized),
                _ => Err(AppError::Validation(
                    "approval status must be one of: pending, approved, rejected, expired, used"
                        .to_string(),
                )),
            }
        })
        .transpose()
}

fn normalize_optional_positive_id(value: Option<i64>, field: &str) -> AppResult<Option<i64>> {
    match value {
        Some(value) if value <= 0 => Err(AppError::Validation(format!("{field} must be positive"))),
        _ => Ok(value),
    }
}

fn parse_replay_mode(value: Option<String>) -> AppResult<ReplayMode> {
    let Some(raw) = value else {
        return Ok(ReplayMode::DryRun);
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "dry_run" => Ok(ReplayMode::DryRun),
        "execute" => Ok(ReplayMode::Execute),
        _ => Err(AppError::Validation(
            "replay mode must be one of: dry_run, execute".to_string(),
        )),
    }
}

fn required_trimmed_token(value: Option<String>) -> AppResult<String> {
    let token = value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "confirmation_token is required for high-risk playbook execution".to_string(),
            )
        })?;

    if token.len() > 128 {
        return Err(AppError::Validation(
            "confirmation_token length must be <= 128".to_string(),
        ));
    }

    Ok(token)
}

fn required_trimmed_approval_token(value: Option<String>) -> AppResult<String> {
    let token = value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "approval_token is required for high-risk playbook execution".to_string(),
            )
        })?;

    if token.len() > 128 {
        return Err(AppError::Validation(
            "approval_token length must be <= 128".to_string(),
        ));
    }

    Ok(token)
}

fn playbook_requires_confirmation(playbook: &PlaybookRecord) -> bool {
    if playbook.requires_confirmation {
        return true;
    }

    matches!(playbook.risk_level.as_str(), "high" | "critical")
}

fn dry_run_risk_summary_text(risk_level: &str, confirmation_required: bool) -> String {
    if confirmation_required {
        format!("Risk level '{risk_level}' requires explicit confirmation after dry-run preview.")
    } else {
        format!("Risk level '{risk_level}' can execute directly after optional dry-run.")
    }
}

fn execution_confirmation_challenge(
    execution: &PlaybookExecutionDetail,
) -> Option<DryRunConfirmationChallenge> {
    let token = execution
        .result
        .get("confirmation_token")
        .and_then(Value::as_str)
        .map(|value| value.to_string());

    let token = token.or_else(|| {
        execution
            .confirmation_required
            .then(|| format!("dry-run-{}", execution.id))
    });

    if !execution.confirmation_required {
        return None;
    }

    Some(DryRunConfirmationChallenge {
        token: token.unwrap_or_else(|| format!("dry-run-{}", execution.id)),
        expires_at: execution.expires_at.unwrap_or_else(Utc::now),
        instruction: "Use this token as confirmation_token with dry_run_id on execute endpoint."
            .to_string(),
    })
}

fn generate_confirmation_token() -> String {
    let uuid = Uuid::new_v4().simple().to_string().to_ascii_uppercase();
    format!("PBK-{}", &uuid[..8])
}

fn generate_approval_token() -> String {
    let uuid = Uuid::new_v4().simple().to_string().to_ascii_uppercase();
    format!("APR-{}", &uuid[..10])
}

fn ensure_playbook_enabled(playbook: &PlaybookRecord) -> AppResult<()> {
    if !playbook.is_enabled {
        return Err(AppError::Validation(format!(
            "playbook '{}' is disabled",
            playbook.key
        )));
    }
    Ok(())
}

fn resolve_actor(headers: &HeaderMap) -> String {
    actor_from_headers(headers)
        .filter(|value| !value.trim().is_empty())
        .map(|value| trim_to_len(value.trim(), MAX_ACTOR_LEN))
        .unwrap_or_else(|| "unknown".to_string())
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trim_to_len(trimmed, max_len))
        }
    })
}

fn trim_to_len(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }

    value.chars().take(max_len).collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use chrono_tz::Tz;
    use serde_json::json;

    use super::{
        ConfirmationDecision, DryRunConfirmationRecord, PlaybookExecutionPolicy,
        PlaybookMaintenanceWindow, PlaybookParameterSchema, dry_run_risk_summary_text,
        evaluate_policy_runtime, normalize_field_type, normalize_optional_approval_status,
        normalize_playbook_key, normalize_playbook_params, parse_parameter_schema,
        required_trimmed_approval_token, validate_confirmation_transition,
    };

    fn sample_schema() -> PlaybookParameterSchema {
        parse_parameter_schema(json!({
            "fields": [
                {"key": "asset_ref", "type": "string", "required": true, "max_length": 10},
                {"key": "grace_seconds", "type": "integer", "required": false, "default": 30, "min": 0, "max": 600},
                {"key": "force", "type": "boolean", "required": false, "default": false},
                {"key": "mode", "type": "enum", "required": false, "options": ["safe", "force"], "default": "safe"}
            ]
        }))
        .expect("schema")
    }

    #[test]
    fn normalizes_playbook_key() {
        assert_eq!(
            normalize_playbook_key(" Restart-Service-Safe ".to_string()).expect("key"),
            "restart-service-safe"
        );
        assert!(normalize_playbook_key("bad key".to_string()).is_err());
    }

    #[test]
    fn validates_supported_field_types() {
        assert_eq!(
            normalize_field_type(" String ".to_string()).expect("field type"),
            "string"
        );
        assert!(normalize_field_type("array".to_string()).is_err());
    }

    #[test]
    fn validates_required_and_unknown_params() {
        let schema = sample_schema();
        let result = normalize_playbook_params(Some(json!({"grace_seconds": 10})), &schema);
        assert!(result.is_err());

        let result =
            normalize_playbook_params(Some(json!({"asset_ref":"srv-a","unexpected":"x"})), &schema);
        assert!(result.is_err());
    }

    #[test]
    fn applies_defaults_and_type_validation() {
        let schema = sample_schema();
        let normalized = normalize_playbook_params(Some(json!({"asset_ref": "srv-a"})), &schema)
            .expect("normalized");

        assert_eq!(
            normalized.get("asset_ref").and_then(|v| v.as_str()),
            Some("srv-a")
        );
        assert_eq!(
            normalized.get("grace_seconds").and_then(|v| v.as_i64()),
            Some(30)
        );
        assert_eq!(
            normalized.get("force").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            normalized.get("mode").and_then(|v| v.as_str()),
            Some("safe")
        );

        let invalid = normalize_playbook_params(
            Some(json!({"asset_ref": "srv-a", "grace_seconds": "oops"})),
            &schema,
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn risk_summary_text_mentions_confirmation_when_required() {
        let summary = dry_run_risk_summary_text("high", true);
        assert!(summary.contains("requires explicit confirmation"));

        let summary = dry_run_risk_summary_text("low", false);
        assert!(summary.contains("can execute directly"));
    }

    #[test]
    fn confirmation_transition_accepts_valid_request() {
        let now = Utc::now();
        let record = DryRunConfirmationRecord {
            id: 100,
            playbook_id: 9,
            actor: "operator-a".to_string(),
            confirmation_token: Some("PBK-ABC12345".to_string()),
            confirmation_required: true,
            expires_at: Some(now + Duration::minutes(30)),
            mode: "dry_run".to_string(),
            status: "planned".to_string(),
        };

        let result =
            validate_confirmation_transition(&record, 9, "operator-a", "PBK-ABC12345", now);
        assert!(result.is_ok());
    }

    #[test]
    fn confirmation_transition_rejects_wrong_token_or_actor_and_marks_expired() {
        let now = Utc::now();
        let base_record = DryRunConfirmationRecord {
            id: 101,
            playbook_id: 9,
            actor: "operator-a".to_string(),
            confirmation_token: Some("PBK-ABC12345".to_string()),
            confirmation_required: true,
            expires_at: Some(now + Duration::minutes(5)),
            mode: "dry_run".to_string(),
            status: "planned".to_string(),
        };

        let wrong_token =
            validate_confirmation_transition(&base_record, 9, "operator-a", "PBK-WRONG", now);
        assert!(wrong_token.is_err());

        let wrong_actor =
            validate_confirmation_transition(&base_record, 9, "viewer-a", "PBK-ABC12345", now);
        assert!(wrong_actor.is_err());

        let expired_record = DryRunConfirmationRecord {
            expires_at: Some(now - Duration::minutes(1)),
            ..base_record
        };
        let expired =
            validate_confirmation_transition(&expired_record, 9, "operator-a", "PBK-ABC12345", now);
        assert!(matches!(expired, Ok(ConfirmationDecision::Expired)));
    }

    #[test]
    fn policy_runtime_blocks_when_outside_window() {
        let policy = PlaybookExecutionPolicy {
            policy_key: "global".to_string(),
            timezone_name: "UTC".to_string(),
            timezone: "UTC".parse::<Tz>().expect("timezone"),
            maintenance_windows: vec![PlaybookMaintenanceWindow {
                day_of_week: 1,
                start: "09:00".to_string(),
                end: "10:00".to_string(),
                label: Some("monday morning".to_string()),
            }],
            change_freeze_enabled: false,
            override_requires_reason: true,
            updated_by: "system".to_string(),
            updated_at: Utc::now(),
        };

        let now = Utc
            .with_ymd_and_hms(2026, 3, 3, 3, 0, 0)
            .single()
            .expect("valid datetime");
        let runtime = evaluate_policy_runtime(&policy, now);
        assert!(!runtime.in_maintenance_window);
        assert!(runtime.blocked_reason.is_some());
        assert!(runtime.next_allowed_at.is_some());
    }

    #[test]
    fn policy_runtime_blocks_when_change_freeze_enabled() {
        let policy = PlaybookExecutionPolicy {
            policy_key: "global".to_string(),
            timezone_name: "UTC".to_string(),
            timezone: "UTC".parse::<Tz>().expect("timezone"),
            maintenance_windows: vec![PlaybookMaintenanceWindow {
                day_of_week: 2,
                start: "00:00".to_string(),
                end: "23:59".to_string(),
                label: None,
            }],
            change_freeze_enabled: true,
            override_requires_reason: true,
            updated_by: "system".to_string(),
            updated_at: Utc::now(),
        };

        let now = Utc
            .with_ymd_and_hms(2026, 3, 3, 6, 0, 0)
            .single()
            .expect("valid datetime");
        let runtime = evaluate_policy_runtime(&policy, now);
        assert!(runtime.blocked_reason.is_some());
        assert!(runtime
            .blocked_reason
            .expect("blocked reason")
            .contains("change-freeze"));
    }

    #[test]
    fn policy_runtime_allows_inside_window_when_not_frozen() {
        let policy = PlaybookExecutionPolicy {
            policy_key: "global".to_string(),
            timezone_name: "UTC".to_string(),
            timezone: "UTC".parse::<Tz>().expect("timezone"),
            maintenance_windows: vec![PlaybookMaintenanceWindow {
                day_of_week: 2,
                start: "00:00".to_string(),
                end: "23:59".to_string(),
                label: Some("all day".to_string()),
            }],
            change_freeze_enabled: false,
            override_requires_reason: true,
            updated_by: "system".to_string(),
            updated_at: Utc::now(),
        };

        let now = Utc
            .with_ymd_and_hms(2026, 3, 3, 6, 0, 0)
            .single()
            .expect("valid datetime");
        let runtime = evaluate_policy_runtime(&policy, now);
        assert!(runtime.in_maintenance_window);
        assert!(runtime.blocked_reason.is_none());
    }

    #[test]
    fn normalizes_approval_status_filter() {
        assert_eq!(
            normalize_optional_approval_status(Some(" PENDING ".to_string())).expect("status"),
            Some("pending".to_string())
        );
        assert!(normalize_optional_approval_status(Some("bad".to_string())).is_err());
    }

    #[test]
    fn validates_required_approval_token() {
        let token = required_trimmed_approval_token(Some("  token-1  ".to_string())).expect("token");
        assert_eq!(token, "token-1");
        assert!(required_trimmed_approval_token(Some("".to_string())).is_err());
        assert!(required_trimmed_approval_token(Some("x".repeat(129))).is_err());
    }
}
