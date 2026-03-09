use std::collections::BTreeSet;
use std::time::{Duration as StdDuration, Instant};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_TEMPLATE_KEY_LEN: usize = 64;
const MAX_TEXT_FIELD_LEN: usize = 256;
const MAX_NOTE_LEN: usize = 1024;
const MAX_PRESET_NAME_LEN: usize = 128;
const MAX_PRESET_DESCRIPTION_LEN: usize = 512;
const MAX_TICKET_REF_LEN: usize = 128;
const MAX_ARTIFACT_URL_LEN: usize = 1024;
const DEFAULT_EXECUTION_LIMIT: u32 = 30;
const MAX_EXECUTION_LIMIT: u32 = 120;
const DEFAULT_PRESET_LIMIT: u32 = 50;
const MAX_PRESET_LIMIT: u32 = 120;
const DEFAULT_EXECUTION_POLICY_KEY: &str = "global";
const EXECUTION_MODE_SIMULATE: &str = "simulate";
const EXECUTION_MODE_LIVE: &str = "live";
const EXECUTION_POLICY_MODE_SIMULATE_ONLY: &str = "simulate_only";
const EXECUTION_POLICY_MODE_HYBRID_LIVE: &str = "hybrid_live";
const DEFAULT_LIVE_STEP_TIMEOUT_SECONDS: i32 = 10;
const MIN_LIVE_STEP_TIMEOUT_SECONDS: i32 = 1;
const MAX_LIVE_STEP_TIMEOUT_SECONDS: i32 = 120;
const MAX_LIVE_TEMPLATE_COUNT: usize = 32;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/runbook-templates", get(list_runbook_templates))
        .route(
            "/cockpit/runbook-templates/presets",
            get(list_runbook_execution_presets).post(create_runbook_execution_preset),
        )
        .route(
            "/cockpit/runbook-templates/presets/{id}",
            patch(update_runbook_execution_preset).delete(delete_runbook_execution_preset),
        )
        .route(
            "/cockpit/runbook-templates/execution-policy",
            get(get_runbook_execution_policy).put(update_runbook_execution_policy),
        )
        .route(
            "/cockpit/runbook-templates/executions",
            get(list_runbook_template_executions),
        )
        .route(
            "/cockpit/runbook-templates/executions/{id}",
            get(get_runbook_template_execution),
        )
        .route(
            "/cockpit/runbook-templates/executions/{id}/replay",
            post(replay_runbook_template_execution),
        )
        .route(
            "/cockpit/runbook-templates/{key}/execute",
            post(execute_runbook_template),
        )
}

#[derive(Debug, Clone)]
struct RunbookTemplateDefinition {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    supports_live: bool,
    params: Vec<RunbookTemplateParamDefinition>,
    preflight: Vec<RunbookTemplateChecklistDefinition>,
    steps: Vec<RunbookTemplateStepDefinition>,
}

#[derive(Debug, Clone)]
struct RunbookTemplateParamDefinition {
    key: &'static str,
    label: &'static str,
    field_type: &'static str,
    required: bool,
    options: Vec<&'static str>,
    min_value: Option<i64>,
    max_value: Option<i64>,
    default_value: Option<&'static str>,
    placeholder: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct RunbookTemplateChecklistDefinition {
    key: &'static str,
    label: &'static str,
    detail: &'static str,
}

#[derive(Debug, Clone)]
struct RunbookTemplateStepDefinition {
    step_id: &'static str,
    name: &'static str,
    detail: &'static str,
    failure_hint: &'static str,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateParamItem {
    key: String,
    label: String,
    field_type: String,
    required: bool,
    options: Vec<String>,
    min_value: Option<i64>,
    max_value: Option<i64>,
    default_value: Option<String>,
    placeholder: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateChecklistItem {
    key: String,
    label: String,
    detail: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateStepItem {
    step_id: String,
    name: String,
    detail: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateCatalogItem {
    key: String,
    name: String,
    description: String,
    category: String,
    execution_modes: Vec<String>,
    params: Vec<RunbookTemplateParamItem>,
    preflight: Vec<RunbookTemplateChecklistItem>,
    steps: Vec<RunbookTemplateStepItem>,
}

#[derive(Debug, Serialize)]
struct ListRunbookTemplatesResponse {
    generated_at: DateTime<Utc>,
    total: usize,
    items: Vec<RunbookTemplateCatalogItem>,
}

#[derive(Debug, Deserialize)]
struct ExecuteRunbookTemplateRequest {
    execution_mode: Option<String>,
    params: Value,
    preflight_confirmations: Vec<String>,
    evidence: RunbookEvidenceInput,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunbookEvidenceInput {
    summary: String,
    ticket_ref: Option<String>,
    artifact_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RunbookStepTimelineEvent {
    step_id: String,
    name: String,
    detail: String,
    status: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    output: String,
    remediation_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RunbookEvidenceRecord {
    summary: String,
    ticket_ref: Option<String>,
    artifact_url: Option<String>,
    captured_at: DateTime<Utc>,
    execution_status: String,
    operator: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateExecutionItem {
    id: i64,
    template_key: String,
    template_name: String,
    status: String,
    execution_mode: String,
    replay_source_execution_id: Option<i64>,
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Vec<RunbookStepTimelineEvent>,
    evidence: RunbookEvidenceRecord,
    runtime_summary: Value,
    remediation_hints: Vec<String>,
    note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RunbookTemplateExecutionRow {
    id: i64,
    template_key: String,
    template_name: String,
    status: String,
    execution_mode: String,
    replay_source_execution_id: Option<i64>,
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Value,
    evidence: Value,
    runtime_summary: Value,
    remediation_hints: Value,
    note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ExecuteRunbookTemplateResponse {
    generated_at: DateTime<Utc>,
    template: RunbookTemplateCatalogItem,
    execution: RunbookTemplateExecutionItem,
}

#[derive(Debug, Deserialize)]
struct ReplayRunbookTemplateExecutionRequest {
    execution_mode: Option<String>,
    evidence: Option<RunbookEvidenceInput>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReplayRunbookTemplateExecutionResponse {
    generated_at: DateTime<Utc>,
    template: RunbookTemplateCatalogItem,
    source_execution_id: i64,
    execution: RunbookTemplateExecutionItem,
}

#[derive(Debug, Deserialize, Default)]
struct ListRunbookTemplateExecutionsQuery {
    template_key: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ListRunbookTemplateExecutionsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<RunbookTemplateExecutionItem>,
}

#[derive(Debug, Serialize)]
struct RunbookTemplateExecutionDetailResponse {
    generated_at: DateTime<Utc>,
    item: RunbookTemplateExecutionItem,
}

#[derive(Debug, FromRow)]
struct RunbookExecutionPolicyRow {
    policy_key: String,
    mode: String,
    live_templates: Value,
    max_live_step_timeout_seconds: i32,
    allow_simulate_failure: bool,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPolicyItem {
    policy_key: String,
    mode: String,
    live_templates: Vec<String>,
    max_live_step_timeout_seconds: i32,
    allow_simulate_failure: bool,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPolicyResponse {
    generated_at: DateTime<Utc>,
    policy: RunbookExecutionPolicyItem,
}

#[derive(Debug, Deserialize)]
struct UpdateRunbookExecutionPolicyRequest {
    mode: Option<String>,
    live_templates: Option<Vec<String>>,
    max_live_step_timeout_seconds: Option<i32>,
    allow_simulate_failure: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, FromRow)]
struct RunbookExecutionPresetRow {
    id: i64,
    template_key: String,
    template_name: String,
    name: String,
    description: Option<String>,
    execution_mode: String,
    params: Value,
    preflight_confirmations: Value,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookExecutionPresetItem {
    id: i64,
    template_key: String,
    template_name: String,
    name: String,
    description: Option<String>,
    execution_mode: String,
    params: Value,
    preflight_confirmations: Vec<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
struct ListRunbookExecutionPresetsQuery {
    template_key: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ListRunbookExecutionPresetsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<RunbookExecutionPresetItem>,
}

#[derive(Debug, Deserialize)]
struct CreateRunbookExecutionPresetRequest {
    template_key: String,
    name: String,
    description: Option<String>,
    execution_mode: Option<String>,
    params: Value,
    preflight_confirmations: Vec<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateRunbookExecutionPresetRequest {
    name: Option<String>,
    description: Option<String>,
    execution_mode: Option<String>,
    params: Option<Value>,
    preflight_confirmations: Option<Vec<String>>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPresetDetailResponse {
    generated_at: DateTime<Utc>,
    item: RunbookExecutionPresetItem,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPresetDeleteResponse {
    generated_at: DateTime<Utc>,
    deleted_id: i64,
}

async fn list_runbook_templates() -> AppResult<Json<ListRunbookTemplatesResponse>> {
    let mut items = built_in_runbook_templates()
        .into_iter()
        .map(|template| runbook_template_to_catalog_item(&template))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.key.cmp(&right.key))
    });

    Ok(Json(ListRunbookTemplatesResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
    }))
}

async fn list_runbook_execution_presets(
    State(state): State<AppState>,
    Query(query): Query<ListRunbookExecutionPresetsQuery>,
) -> AppResult<Json<ListRunbookExecutionPresetsResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_PRESET_LIMIT)
        .clamp(1, MAX_PRESET_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_runbook_execution_presets p WHERE 1=1");
    append_preset_filters(&mut count_builder, template_key.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, name, description, execution_mode,
                params, preflight_confirmations, updated_by, created_at, updated_at
         FROM ops_runbook_execution_presets p
         WHERE 1=1",
    );
    append_preset_filters(&mut list_builder, template_key);
    list_builder
        .push(" ORDER BY p.template_key ASC, p.name ASC, p.id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RunbookExecutionPresetRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    let mut items = Vec::new();
    for row in rows {
        items.push(parse_runbook_execution_preset_row(row)?);
    }

    Ok(Json(ListRunbookExecutionPresetsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn create_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRunbookExecutionPresetRequest>,
) -> AppResult<Json<RunbookExecutionPresetDetailResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template = resolve_template_by_key(payload.template_key.as_str())?;
    let name = normalize_preset_name(payload.name)?;
    let description = trim_optional(payload.description, MAX_PRESET_DESCRIPTION_LEN);
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| EXECUTION_MODE_SIMULATE.to_string()),
    )?;
    enforce_template_supports_execution_mode(&template, execution_mode.as_str())?;
    let params = normalize_runbook_params(&template, payload.params)?;
    let preflight_confirmations =
        normalize_preflight_confirmations(&template, payload.preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_runbook_execution_presets
         WHERE template_key = $1
           AND name = $2",
    )
    .bind(template.key)
    .bind(name.as_str())
    .fetch_optional(&state.db)
    .await?;
    if existing.is_some() {
        return Err(AppError::Validation(format!(
            "runbook preset '{}' already exists for template '{}'",
            name, template.key
        )));
    }

    let row: RunbookExecutionPresetRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_presets (
            template_key, template_name, name, description, execution_mode,
            params, preflight_confirmations, updated_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(name.as_str())
    .bind(description.clone())
    .bind(execution_mode.as_str())
    .bind(Value::Object(params.clone()))
    .bind(
        serde_json::to_value(&preflight_confirmations).map_err(|err| {
            AppError::Validation(format!(
                "failed to serialize preset preflight_confirmations: {err}"
            ))
        })?,
    )
    .bind(actor.as_str())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.preset.create".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "preset_name": name,
                "execution_mode": execution_mode,
                "param_count": params.len(),
                "preflight_count": preflight_confirmations.len()
            }),
        },
    )
    .await;

    let item = parse_runbook_execution_preset_row(row)?;
    Ok(Json(RunbookExecutionPresetDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

async fn update_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateRunbookExecutionPresetRequest>,
) -> AppResult<Json<RunbookExecutionPresetDetailResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "preset id must be a positive integer".to_string(),
        ));
    }

    let has_mutation = payload.name.is_some()
        || payload.description.is_some()
        || payload.execution_mode.is_some()
        || payload.params.is_some()
        || payload.preflight_confirmations.is_some();
    if !has_mutation {
        return Err(AppError::Validation(
            "at least one preset field must be provided".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let existing: Option<RunbookExecutionPresetRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, name, description, execution_mode,
                params, preflight_confirmations, updated_by, created_at, updated_at
         FROM ops_runbook_execution_presets
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let existing =
        existing.ok_or_else(|| AppError::NotFound(format!("runbook preset {id} not found")))?;

    let template = resolve_template_by_key(existing.template_key.as_str())?;
    let name = match payload.name {
        Some(value) => normalize_preset_name(value)?,
        None => existing.name.clone(),
    };
    let description = if payload.description.is_some() {
        trim_optional(payload.description, MAX_PRESET_DESCRIPTION_LEN)
    } else {
        existing.description.clone()
    };
    let execution_mode = payload
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?
        .unwrap_or(existing.execution_mode.clone());
    enforce_template_supports_execution_mode(&template, execution_mode.as_str())?;
    let params = match payload.params {
        Some(value) => normalize_runbook_params(&template, value)?,
        None => parse_params_object(existing.params.clone())?,
    };
    let preflight_confirmations = match payload.preflight_confirmations {
        Some(value) => normalize_preflight_confirmations(&template, value)?,
        None => parse_preflight_confirmations(existing.preflight_confirmations.clone())?,
    };
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let duplicate: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_runbook_execution_presets
         WHERE template_key = $1
           AND name = $2
           AND id <> $3",
    )
    .bind(template.key)
    .bind(name.as_str())
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    if duplicate.is_some() {
        return Err(AppError::Validation(format!(
            "runbook preset '{}' already exists for template '{}'",
            name, template.key
        )));
    }

    let row: RunbookExecutionPresetRow = sqlx::query_as(
        "UPDATE ops_runbook_execution_presets
         SET name = $1,
             description = $2,
             execution_mode = $3,
             params = $4,
             preflight_confirmations = $5,
             updated_by = $6,
             updated_at = NOW()
         WHERE id = $7
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(name.as_str())
    .bind(description.clone())
    .bind(execution_mode.as_str())
    .bind(Value::Object(params.clone()))
    .bind(
        serde_json::to_value(&preflight_confirmations).map_err(|err| {
            AppError::Validation(format!(
                "failed to serialize preset preflight_confirmations: {err}"
            ))
        })?,
    )
    .bind(actor.as_str())
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.preset.update".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "preset_name": name,
                "execution_mode": execution_mode,
                "param_count": params.len(),
                "preflight_count": preflight_confirmations.len()
            }),
        },
    )
    .await;

    let item = parse_runbook_execution_preset_row(row)?;
    Ok(Json(RunbookExecutionPresetDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

async fn delete_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<RunbookExecutionPresetDeleteResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "preset id must be a positive integer".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let row: Option<RunbookExecutionPresetRow> = sqlx::query_as(
        "DELETE FROM ops_runbook_execution_presets
         WHERE id = $1
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("runbook preset {id} not found")))?;
    let note = Some(format!("deleted runbook preset '{}'", row.name));

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.preset.delete".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": row.template_key,
                "preset_name": row.name,
                "execution_mode": row.execution_mode
            }),
        },
    )
    .await;

    Ok(Json(RunbookExecutionPresetDeleteResponse {
        generated_at: Utc::now(),
        deleted_id: id,
    }))
}

async fn get_runbook_execution_policy(
    State(state): State<AppState>,
) -> AppResult<Json<RunbookExecutionPolicyResponse>> {
    let row = load_or_seed_execution_policy(&state).await?;
    let policy = parse_execution_policy_row(row)?;
    Ok(Json(RunbookExecutionPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn update_runbook_execution_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRunbookExecutionPolicyRequest>,
) -> AppResult<Json<RunbookExecutionPolicyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;

    let mode = payload
        .mode
        .map(normalize_execution_policy_mode)
        .transpose()?
        .unwrap_or(current.mode.clone());
    let live_templates = payload
        .live_templates
        .map(normalize_live_template_keys)
        .transpose()?
        .unwrap_or_else(|| current.live_templates.clone());
    let max_live_step_timeout_seconds = payload
        .max_live_step_timeout_seconds
        .unwrap_or(current.max_live_step_timeout_seconds);
    let allow_simulate_failure = payload
        .allow_simulate_failure
        .unwrap_or(current.allow_simulate_failure);
    let note = match payload.note {
        Some(value) => trim_optional(Some(value), MAX_NOTE_LEN),
        None => current.note.clone(),
    };

    if !(MIN_LIVE_STEP_TIMEOUT_SECONDS..=MAX_LIVE_STEP_TIMEOUT_SECONDS)
        .contains(&max_live_step_timeout_seconds)
    {
        return Err(AppError::Validation(format!(
            "max_live_step_timeout_seconds must be between {} and {}",
            MIN_LIVE_STEP_TIMEOUT_SECONDS, MAX_LIVE_STEP_TIMEOUT_SECONDS
        )));
    }
    if mode == EXECUTION_POLICY_MODE_HYBRID_LIVE && live_templates.is_empty() {
        return Err(AppError::Validation(
            "live_templates cannot be empty when mode=hybrid_live".to_string(),
        ));
    }

    let row: RunbookExecutionPolicyRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_policies (
            policy_key, mode, live_templates, max_live_step_timeout_seconds,
            allow_simulate_failure, note, updated_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (policy_key) DO UPDATE
         SET mode = EXCLUDED.mode,
             live_templates = EXCLUDED.live_templates,
             max_live_step_timeout_seconds = EXCLUDED.max_live_step_timeout_seconds,
             allow_simulate_failure = EXCLUDED.allow_simulate_failure,
             note = EXCLUDED.note,
             updated_by = EXCLUDED.updated_by,
             updated_at = NOW()
         RETURNING policy_key, mode, live_templates, max_live_step_timeout_seconds,
                   allow_simulate_failure, note, updated_by, created_at, updated_at",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .bind(mode.as_str())
    .bind(serde_json::to_value(&live_templates).map_err(|err| {
        AppError::Validation(format!("failed to serialize live_templates: {err}"))
    })?)
    .bind(max_live_step_timeout_seconds)
    .bind(allow_simulate_failure)
    .bind(note.clone())
    .bind(actor.as_str())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.execution_policy.update".to_string(),
            target_type: "ops_runbook_execution_policy".to_string(),
            target_id: Some(DEFAULT_EXECUTION_POLICY_KEY.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "mode": mode,
                "live_template_count": live_templates.len(),
                "max_live_step_timeout_seconds": max_live_step_timeout_seconds,
                "allow_simulate_failure": allow_simulate_failure
            }),
        },
    )
    .await;

    let policy = parse_execution_policy_row(row)?;
    Ok(Json(RunbookExecutionPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn execute_runbook_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<ExecuteRunbookTemplateRequest>,
) -> AppResult<Json<ExecuteRunbookTemplateResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template = resolve_template_by_key(key.as_str())?;
    let execution_policy =
        parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| EXECUTION_MODE_SIMULATE.to_string()),
    )?;
    enforce_execution_mode_policy(&execution_policy, &template, execution_mode.as_str())?;

    let normalized_params = normalize_runbook_params(&template, payload.params)?;
    if execution_mode == EXECUTION_MODE_LIVE
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is not allowed in live execution mode".to_string(),
        ));
    }
    if execution_mode == EXECUTION_MODE_SIMULATE
        && !execution_policy.allow_simulate_failure
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is disabled by execution policy".to_string(),
        ));
    }

    let preflight_confirmations =
        normalize_preflight_confirmations(&template, payload.preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);
    let evidence_input = normalize_runbook_evidence_input(payload.evidence)?;

    let outcome = if execution_mode == EXECUTION_MODE_LIVE {
        execute_live_template(
            &template,
            &normalized_params,
            execution_policy.max_live_step_timeout_seconds,
        )
        .await?
    } else {
        execute_simulated_template(&template, &normalized_params)
    };
    let final_status = outcome.status.clone();

    let evidence = RunbookEvidenceRecord {
        summary: evidence_input.summary,
        ticket_ref: evidence_input.ticket_ref,
        artifact_url: evidence_input.artifact_url,
        captured_at: Utc::now(),
        execution_status: final_status.clone(),
        operator: actor.clone(),
    };

    let preflight_snapshot = json!({
        "confirmed": preflight_confirmations,
        "total_required": template.preflight.len(),
    });

    let row: RunbookTemplateExecutionRow = sqlx::query_as(
        "INSERT INTO ops_runbook_template_executions (
            template_key,
            template_name,
            status,
            execution_mode,
            replay_source_execution_id,
            actor,
            params,
            preflight,
            timeline,
            evidence,
            runtime_summary,
            remediation_hints,
            note
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                   actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                   note, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(final_status.as_str())
    .bind(execution_mode.as_str())
    .bind(None::<i64>)
    .bind(actor.as_str())
    .bind(Value::Object(normalized_params.clone()))
    .bind(preflight_snapshot)
    .bind(serde_json::to_value(&outcome.timeline).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook timeline: {err}"))
    })?)
    .bind(serde_json::to_value(&evidence).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook evidence: {err}"))
    })?)
    .bind(outcome.runtime_summary.clone())
    .bind(serde_json::to_value(&outcome.remediation_hints).map_err(|err| {
        AppError::Validation(format!("failed to serialize remediation hints: {err}"))
    })?)
    .bind(note.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.template.execute".to_string(),
            target_type: "ops_runbook_template_execution".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "status": final_status,
                "execution_mode": execution_mode,
                "step_count": outcome.timeline.len(),
                "remediation_hint_count": outcome.remediation_hints.len(),
                "policy_mode": execution_policy.mode,
            }),
        },
    )
    .await;

    let execution = parse_execution_row(row)?;

    Ok(Json(ExecuteRunbookTemplateResponse {
        generated_at: execution.created_at,
        template: runbook_template_to_catalog_item(&template),
        execution,
    }))
}

async fn replay_runbook_template_execution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<ReplayRunbookTemplateExecutionRequest>,
) -> AppResult<Json<ReplayRunbookTemplateExecutionResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "execution id must be a positive integer".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let source_row: Option<RunbookTemplateExecutionRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let source_row = source_row
        .ok_or_else(|| AppError::NotFound(format!("runbook execution {id} not found")))?;
    let source_execution = parse_execution_row(source_row)?;

    let template = resolve_template_by_key(source_execution.template_key.as_str())?;
    let execution_policy =
        parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| source_execution.execution_mode.clone()),
    )?;
    enforce_execution_mode_policy(&execution_policy, &template, execution_mode.as_str())?;

    let normalized_params = normalize_runbook_params(&template, source_execution.params.clone())?;
    if execution_mode == EXECUTION_MODE_LIVE
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is not allowed in live execution mode".to_string(),
        ));
    }
    if execution_mode == EXECUTION_MODE_SIMULATE
        && !execution_policy.allow_simulate_failure
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is disabled by execution policy".to_string(),
        ));
    }

    let source_preflight_confirmations =
        parse_preflight_snapshot_confirmed(source_execution.preflight.clone())?;
    let preflight_confirmations =
        normalize_preflight_confirmations(&template, source_preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let source_evidence = source_execution.evidence;
    let evidence_input = match payload.evidence {
        Some(value) => normalize_runbook_evidence_input(value)?,
        None => normalize_runbook_evidence_input(RunbookEvidenceInput {
            summary: source_evidence.summary,
            ticket_ref: source_evidence.ticket_ref,
            artifact_url: source_evidence.artifact_url,
        })?,
    };

    let mut outcome = if execution_mode == EXECUTION_MODE_LIVE {
        execute_live_template(
            &template,
            &normalized_params,
            execution_policy.max_live_step_timeout_seconds,
        )
        .await?
    } else {
        execute_simulated_template(&template, &normalized_params)
    };
    if let Value::Object(runtime_summary) = &mut outcome.runtime_summary {
        runtime_summary.insert("replay_source_execution_id".to_string(), json!(id));
    }
    let final_status = outcome.status.clone();

    let evidence = RunbookEvidenceRecord {
        summary: evidence_input.summary,
        ticket_ref: evidence_input.ticket_ref,
        artifact_url: evidence_input.artifact_url,
        captured_at: Utc::now(),
        execution_status: final_status.clone(),
        operator: actor.clone(),
    };
    let preflight_snapshot = json!({
        "confirmed": preflight_confirmations,
        "total_required": template.preflight.len(),
    });

    let row: RunbookTemplateExecutionRow = sqlx::query_as(
        "INSERT INTO ops_runbook_template_executions (
            template_key,
            template_name,
            status,
            execution_mode,
            replay_source_execution_id,
            actor,
            params,
            preflight,
            timeline,
            evidence,
            runtime_summary,
            remediation_hints,
            note
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                   actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                   note, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(final_status.as_str())
    .bind(execution_mode.as_str())
    .bind(id)
    .bind(actor.as_str())
    .bind(Value::Object(normalized_params.clone()))
    .bind(preflight_snapshot)
    .bind(serde_json::to_value(&outcome.timeline).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook timeline: {err}"))
    })?)
    .bind(serde_json::to_value(&evidence).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook evidence: {err}"))
    })?)
    .bind(outcome.runtime_summary.clone())
    .bind(serde_json::to_value(&outcome.remediation_hints).map_err(|err| {
        AppError::Validation(format!("failed to serialize remediation hints: {err}"))
    })?)
    .bind(note.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.template.replay".to_string(),
            target_type: "ops_runbook_template_execution".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "status": final_status,
                "execution_mode": execution_mode,
                "step_count": outcome.timeline.len(),
                "remediation_hint_count": outcome.remediation_hints.len(),
                "policy_mode": execution_policy.mode,
                "replay_source_execution_id": id
            }),
        },
    )
    .await;

    let execution = parse_execution_row(row)?;
    Ok(Json(ReplayRunbookTemplateExecutionResponse {
        generated_at: execution.created_at,
        template: runbook_template_to_catalog_item(&template),
        source_execution_id: id,
        execution,
    }))
}

async fn list_runbook_template_executions(
    State(state): State<AppState>,
    Query(query): Query<ListRunbookTemplateExecutionsQuery>,
) -> AppResult<Json<ListRunbookTemplateExecutionsResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let status = query.status.map(normalize_execution_status).transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_runbook_template_executions e WHERE 1=1");
    append_execution_filters(&mut count_builder, template_key.clone(), status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE 1=1",
    );
    append_execution_filters(&mut list_builder, template_key, status);
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    let mut items = Vec::new();
    for row in rows {
        items.push(parse_execution_row(row)?);
    }

    Ok(Json(ListRunbookTemplateExecutionsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn get_runbook_template_execution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<RunbookTemplateExecutionDetailResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "execution id must be a positive integer".to_string(),
        ));
    }

    let row: Option<RunbookTemplateExecutionRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound(format!("runbook execution {id} not found")))?;
    let item = parse_execution_row(row)?;

    Ok(Json(RunbookTemplateExecutionDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

fn append_execution_filters(
    builder: &mut QueryBuilder<Postgres>,
    template_key: Option<String>,
    status: Option<String>,
) {
    if let Some(template_key) = template_key {
        builder
            .push(" AND e.template_key = ")
            .push_bind(template_key);
    }
    if let Some(status) = status {
        builder.push(" AND e.status = ").push_bind(status);
    }
}

fn append_preset_filters(builder: &mut QueryBuilder<Postgres>, template_key: Option<String>) {
    if let Some(template_key) = template_key {
        builder
            .push(" AND p.template_key = ")
            .push_bind(template_key);
    }
}

fn parse_runbook_execution_preset_row(
    row: RunbookExecutionPresetRow,
) -> AppResult<RunbookExecutionPresetItem> {
    let preflight_confirmations = parse_preflight_confirmations(row.preflight_confirmations)?;
    let execution_mode = normalize_execution_mode(row.execution_mode)?;
    if !matches!(row.params, Value::Object(_)) {
        return Err(AppError::Validation(
            "runbook preset params must be a JSON object".to_string(),
        ));
    }

    Ok(RunbookExecutionPresetItem {
        id: row.id,
        template_key: row.template_key,
        template_name: row.template_name,
        name: row.name,
        description: row.description,
        execution_mode,
        params: row.params,
        preflight_confirmations,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn parse_execution_row(
    row: RunbookTemplateExecutionRow,
) -> AppResult<RunbookTemplateExecutionItem> {
    let timeline: Vec<RunbookStepTimelineEvent> =
        serde_json::from_value(row.timeline).map_err(|err| {
            AppError::Validation(format!("runbook execution timeline data is invalid: {err}"))
        })?;
    let evidence: RunbookEvidenceRecord = serde_json::from_value(row.evidence).map_err(|err| {
        AppError::Validation(format!("runbook execution evidence data is invalid: {err}"))
    })?;
    let remediation_hints: Vec<String> =
        serde_json::from_value(row.remediation_hints).map_err(|err| {
            AppError::Validation(format!(
                "runbook execution remediation_hints data is invalid: {err}"
            ))
        })?;

    Ok(RunbookTemplateExecutionItem {
        id: row.id,
        template_key: row.template_key,
        template_name: row.template_name,
        status: row.status,
        execution_mode: row.execution_mode,
        replay_source_execution_id: row.replay_source_execution_id,
        actor: row.actor,
        params: row.params,
        preflight: row.preflight,
        timeline,
        evidence,
        runtime_summary: row.runtime_summary,
        remediation_hints,
        note: row.note,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[derive(Debug)]
struct RunbookExecutionOutcome {
    status: String,
    timeline: Vec<RunbookStepTimelineEvent>,
    remediation_hints: Vec<String>,
    runtime_summary: Value,
}

#[derive(Debug)]
struct LiveProbeTarget {
    host: String,
    port: u16,
    raw: String,
}

async fn load_or_seed_execution_policy(state: &AppState) -> AppResult<RunbookExecutionPolicyRow> {
    let existing: Option<RunbookExecutionPolicyRow> = sqlx::query_as(
        "SELECT policy_key, mode, live_templates, max_live_step_timeout_seconds,
                allow_simulate_failure, note, updated_by, created_at, updated_at
         FROM ops_runbook_execution_policies
         WHERE policy_key = $1",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = existing {
        return Ok(row);
    }

    let row: RunbookExecutionPolicyRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_policies (
            policy_key, mode, live_templates, max_live_step_timeout_seconds,
            allow_simulate_failure, note, updated_by
         )
         VALUES ($1, $2, '[]'::jsonb, $3, $4, $5, $6)
         RETURNING policy_key, mode, live_templates, max_live_step_timeout_seconds,
                   allow_simulate_failure, note, updated_by, created_at, updated_at",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .bind(EXECUTION_POLICY_MODE_SIMULATE_ONLY)
    .bind(DEFAULT_LIVE_STEP_TIMEOUT_SECONDS)
    .bind(true)
    .bind(Some("seeded default policy".to_string()))
    .bind("system")
    .fetch_one(&state.db)
    .await?;

    Ok(row)
}

fn parse_execution_policy_row(
    row: RunbookExecutionPolicyRow,
) -> AppResult<RunbookExecutionPolicyItem> {
    let mode = normalize_execution_policy_mode(row.mode)?;
    let live_templates: Vec<String> =
        serde_json::from_value(row.live_templates).map_err(|err| {
            AppError::Validation(format!(
                "runbook execution policy live_templates is invalid: {err}"
            ))
        })?;
    let live_templates = normalize_live_template_keys(live_templates)?;
    if !(MIN_LIVE_STEP_TIMEOUT_SECONDS..=MAX_LIVE_STEP_TIMEOUT_SECONDS)
        .contains(&row.max_live_step_timeout_seconds)
    {
        return Err(AppError::Validation(format!(
            "runbook execution policy timeout must be between {} and {} seconds",
            MIN_LIVE_STEP_TIMEOUT_SECONDS, MAX_LIVE_STEP_TIMEOUT_SECONDS
        )));
    }

    Ok(RunbookExecutionPolicyItem {
        policy_key: row.policy_key,
        mode,
        live_templates,
        max_live_step_timeout_seconds: row.max_live_step_timeout_seconds,
        allow_simulate_failure: row.allow_simulate_failure,
        note: row.note,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn normalize_execution_policy_mode(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        EXECUTION_POLICY_MODE_SIMULATE_ONLY | EXECUTION_POLICY_MODE_HYBRID_LIVE => Ok(normalized),
        _ => Err(AppError::Validation(
            "mode must be one of: simulate_only, hybrid_live".to_string(),
        )),
    }
}

fn normalize_execution_mode(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        EXECUTION_MODE_SIMULATE | EXECUTION_MODE_LIVE => Ok(normalized),
        _ => Err(AppError::Validation(
            "execution_mode must be one of: simulate, live".to_string(),
        )),
    }
}

fn normalize_preset_name(value: String) -> AppResult<String> {
    required_trimmed("preset name", value, MAX_PRESET_NAME_LEN)
}

fn enforce_template_supports_execution_mode(
    template: &RunbookTemplateDefinition,
    execution_mode: &str,
) -> AppResult<()> {
    if execution_mode == EXECUTION_MODE_LIVE && !template.supports_live {
        return Err(AppError::Validation(format!(
            "template '{}' does not support live execution",
            template.key
        )));
    }
    Ok(())
}

fn normalize_live_template_keys(raw: Vec<String>) -> AppResult<Vec<String>> {
    if raw.len() > MAX_LIVE_TEMPLATE_COUNT {
        return Err(AppError::Validation(format!(
            "live_templates count must be <= {}",
            MAX_LIVE_TEMPLATE_COUNT
        )));
    }

    let mut dedup = BTreeSet::new();
    for key in raw {
        dedup.insert(normalize_template_key(key)?);
    }

    let mut normalized = dedup.into_iter().collect::<Vec<_>>();
    normalized.sort();

    for key in &normalized {
        let template = resolve_template_by_key(key)?;
        if !template.supports_live {
            return Err(AppError::Validation(format!(
                "template '{}' does not support live execution",
                key
            )));
        }
    }

    Ok(normalized)
}

fn enforce_execution_mode_policy(
    policy: &RunbookExecutionPolicyItem,
    template: &RunbookTemplateDefinition,
    execution_mode: &str,
) -> AppResult<()> {
    if execution_mode == EXECUTION_MODE_SIMULATE {
        return Ok(());
    }

    if policy.mode != EXECUTION_POLICY_MODE_HYBRID_LIVE {
        return Err(AppError::Validation(
            "live execution is disabled by execution policy".to_string(),
        ));
    }
    if !template.supports_live {
        return Err(AppError::Validation(format!(
            "template '{}' does not support live execution",
            template.key
        )));
    }
    if !policy
        .live_templates
        .iter()
        .any(|item| item == template.key)
    {
        return Err(AppError::Validation(format!(
            "template '{}' is not allowlisted for live execution",
            template.key
        )));
    }

    Ok(())
}

fn runbook_template_execution_modes(template: &RunbookTemplateDefinition) -> Vec<String> {
    if template.supports_live {
        vec![
            EXECUTION_MODE_SIMULATE.to_string(),
            EXECUTION_MODE_LIVE.to_string(),
        ]
    } else {
        vec![EXECUTION_MODE_SIMULATE.to_string()]
    }
}

fn execute_simulated_template(
    template: &RunbookTemplateDefinition,
    normalized_params: &JsonMap<String, Value>,
) -> RunbookExecutionOutcome {
    let now = Utc::now();
    let mut timeline = Vec::new();
    let mut remediation_hints = Vec::new();
    let mut final_status = "succeeded".to_string();

    for (idx, step) in template.steps.iter().enumerate() {
        let started_at = now + Duration::seconds(idx as i64);
        let finished_at = started_at + Duration::seconds(1);
        let failure_reason =
            evaluate_step_failure(template.key, step.step_id, normalized_params, idx);

        if let Some(reason) = failure_reason {
            final_status = "failed".to_string();
            remediation_hints.push(step.failure_hint.to_string());
            timeline.push(RunbookStepTimelineEvent {
                step_id: step.step_id.to_string(),
                name: step.name.to_string(),
                detail: step.detail.to_string(),
                status: "failed".to_string(),
                started_at,
                finished_at,
                output: reason,
                remediation_hint: Some(step.failure_hint.to_string()),
            });
            break;
        }

        timeline.push(RunbookStepTimelineEvent {
            step_id: step.step_id.to_string(),
            name: step.name.to_string(),
            detail: step.detail.to_string(),
            status: "succeeded".to_string(),
            started_at,
            finished_at,
            output: format!("step '{}' completed", step.name),
            remediation_hint: None,
        });
    }

    if final_status == "failed" && remediation_hints.is_empty() {
        remediation_hints.push(
            "Review failed step output, verify prerequisite checks, and rerun with guarded scope."
                .to_string(),
        );
    }

    let failed_step_id = timeline
        .iter()
        .find(|item| item.status == "failed")
        .map(|item| item.step_id.clone());
    let runtime_summary = json!({
        "mode": EXECUTION_MODE_SIMULATE,
        "total_steps": template.steps.len(),
        "executed_steps": timeline.len(),
        "failed_step_id": failed_step_id,
        "duration_ms": (timeline.len() as i64) * 1000
    });

    RunbookExecutionOutcome {
        status: final_status,
        timeline,
        remediation_hints,
        runtime_summary,
    }
}

async fn execute_live_template(
    template: &RunbookTemplateDefinition,
    normalized_params: &JsonMap<String, Value>,
    max_live_step_timeout_seconds: i32,
) -> AppResult<RunbookExecutionOutcome> {
    if template.key != "dependency-check" {
        return Err(AppError::Validation(format!(
            "template '{}' live execution adapter is not implemented",
            template.key
        )));
    }

    let dependency_target =
        string_param(normalized_params, "dependency_target").ok_or_else(|| {
            AppError::Validation("parameter 'dependency_target' is required".to_string())
        })?;
    let probe_target = parse_dependency_target(dependency_target.as_str())?;
    let configured_timeout_seconds = number_param(normalized_params, "probe_timeout_seconds")
        .unwrap_or(max_live_step_timeout_seconds as i64)
        .clamp(
            MIN_LIVE_STEP_TIMEOUT_SECONDS as i64,
            max_live_step_timeout_seconds as i64,
        ) as u64;

    let mut timeline = Vec::new();
    let mut remediation_hints = Vec::new();
    let mut status = "succeeded".to_string();
    let start_instant = Instant::now();
    let mut probe_latency_ms: Option<i64> = None;

    let validation_started_at = Utc::now();
    let validation_finished_at = validation_started_at + Duration::milliseconds(50);
    timeline.push(RunbookStepTimelineEvent {
        step_id: "scope_validation".to_string(),
        name: "Validate dependency scope".to_string(),
        detail: "Confirm dependency target syntax and authorized probing scope.".to_string(),
        status: "succeeded".to_string(),
        started_at: validation_started_at,
        finished_at: validation_finished_at,
        output: format!(
            "resolved target {}:{}",
            probe_target.host, probe_target.port
        ),
        remediation_hint: None,
    });

    let probe_started_at = Utc::now();
    let probe_clock = Instant::now();
    let probe_result = timeout(
        StdDuration::from_secs(configured_timeout_seconds),
        TcpStream::connect((probe_target.host.as_str(), probe_target.port)),
    )
    .await;
    let probe_finished_at = Utc::now();

    match probe_result {
        Ok(Ok(stream)) => {
            let elapsed_ms = probe_clock.elapsed().as_millis() as i64;
            probe_latency_ms = Some(elapsed_ms);
            drop(stream);
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "succeeded".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe succeeded target={}:{} latency_ms={}",
                    probe_target.host, probe_target.port, elapsed_ms
                ),
                remediation_hint: None,
            });
        }
        Ok(Err(err)) => {
            status = "failed".to_string();
            remediation_hints.push(
                "Check network ACL/DNS path and retry with dependency owner support.".to_string(),
            );
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "failed".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe failed target={}:{} error={}",
                    probe_target.host, probe_target.port, err
                ),
                remediation_hint: Some(
                    "Check network ACL/DNS path and retry with dependency owner support."
                        .to_string(),
                ),
            });
        }
        Err(_) => {
            status = "failed".to_string();
            remediation_hints.push(
                "Dependency probe timed out; validate firewall path and endpoint health."
                    .to_string(),
            );
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "failed".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe timeout target={}:{} timeout_seconds={}",
                    probe_target.host, probe_target.port, configured_timeout_seconds
                ),
                remediation_hint: Some(
                    "Dependency probe timed out; validate firewall path and endpoint health."
                        .to_string(),
                ),
            });
        }
    }

    if status == "succeeded" {
        let summary_started_at = Utc::now();
        let summary_finished_at = summary_started_at + Duration::milliseconds(25);
        timeline.push(RunbookStepTimelineEvent {
            step_id: "readiness_summary".to_string(),
            name: "Summarize dependency readiness".to_string(),
            detail: "Publish readiness result with latency and error context.".to_string(),
            status: "succeeded".to_string(),
            started_at: summary_started_at,
            finished_at: summary_finished_at,
            output: format!(
                "dependency target {}: {}",
                probe_target.raw,
                probe_latency_ms
                    .map(|value| format!("reachable latency_ms={value}"))
                    .unwrap_or_else(|| "reachable".to_string())
            ),
            remediation_hint: None,
        });
    }

    if status == "failed" && remediation_hints.is_empty() {
        remediation_hints.push(
            "Review failed live probe output and rerun after dependency path remediation."
                .to_string(),
        );
    }

    let failed_step_id = timeline
        .iter()
        .find(|item| item.status == "failed")
        .map(|item| item.step_id.clone());
    let runtime_summary = json!({
        "mode": EXECUTION_MODE_LIVE,
        "total_steps": template.steps.len(),
        "executed_steps": timeline.len(),
        "failed_step_id": failed_step_id,
        "duration_ms": start_instant.elapsed().as_millis() as i64,
        "probe_target": format!("{}:{}", probe_target.host, probe_target.port),
        "probe_latency_ms": probe_latency_ms,
        "probe_timeout_seconds": configured_timeout_seconds
    });

    Ok(RunbookExecutionOutcome {
        status,
        timeline,
        remediation_hints,
        runtime_summary,
    })
}

fn parse_dependency_target(raw: &str) -> AppResult<LiveProbeTarget> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "dependency_target cannot be empty".to_string(),
        ));
    }

    if let Some(rest) = trimmed.strip_prefix("http://") {
        return parse_host_port_target(rest, 80, trimmed);
    }
    if let Some(rest) = trimmed.strip_prefix("https://") {
        return parse_host_port_target(rest, 443, trimmed);
    }

    parse_host_port_target(trimmed, 0, trimmed)
}

fn parse_params_object(value: Value) -> AppResult<JsonMap<String, Value>> {
    let Value::Object(params) = value else {
        return Err(AppError::Validation(
            "runbook preset params must be a JSON object".to_string(),
        ));
    };
    Ok(params)
}

fn parse_preflight_confirmations(value: Value) -> AppResult<Vec<String>> {
    let preflight_confirmations: Vec<String> = serde_json::from_value(value).map_err(|err| {
        AppError::Validation(format!(
            "runbook preset preflight_confirmations is invalid: {err}"
        ))
    })?;
    Ok(preflight_confirmations)
}

fn parse_preflight_snapshot_confirmed(value: Value) -> AppResult<Vec<String>> {
    let Value::Object(snapshot) = value else {
        return Err(AppError::Validation(
            "runbook execution preflight snapshot must be a JSON object".to_string(),
        ));
    };
    let confirmed = snapshot.get("confirmed").cloned().ok_or_else(|| {
        AppError::Validation(
            "runbook execution preflight snapshot missing confirmed field".to_string(),
        )
    })?;
    let confirmed: Vec<String> = serde_json::from_value(confirmed).map_err(|err| {
        AppError::Validation(format!(
            "runbook execution preflight snapshot confirmed field is invalid: {err}"
        ))
    })?;
    Ok(confirmed)
}

fn parse_host_port_target(
    authority_or_host: &str,
    default_port: u16,
    raw: &str,
) -> AppResult<LiveProbeTarget> {
    let authority = authority_or_host.split('/').next().unwrap_or("").trim();
    if authority.is_empty() {
        return Err(AppError::Validation(
            "dependency_target host is required".to_string(),
        ));
    }

    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| {
            AppError::Validation("dependency_target port must be a valid integer".to_string())
        })?;
        (host.trim().to_string(), port)
    } else if default_port > 0 {
        (authority.to_string(), default_port)
    } else {
        return Err(AppError::Validation(
            "dependency_target must include ':<port>' or use http(s):// scheme".to_string(),
        ));
    };

    if host.is_empty() {
        return Err(AppError::Validation(
            "dependency_target host is required".to_string(),
        ));
    }
    if port == 0 {
        return Err(AppError::Validation(
            "dependency_target port must be > 0".to_string(),
        ));
    }

    Ok(LiveProbeTarget {
        host,
        port,
        raw: raw.to_string(),
    })
}

fn resolve_template_by_key(key: &str) -> AppResult<RunbookTemplateDefinition> {
    let normalized = normalize_template_key(key.to_string())?;
    built_in_runbook_templates()
        .into_iter()
        .find(|template| template.key == normalized)
        .ok_or_else(|| AppError::NotFound(format!("runbook template '{}' not found", normalized)))
}

fn runbook_template_to_catalog_item(
    template: &RunbookTemplateDefinition,
) -> RunbookTemplateCatalogItem {
    RunbookTemplateCatalogItem {
        key: template.key.to_string(),
        name: template.name.to_string(),
        description: template.description.to_string(),
        category: template.category.to_string(),
        execution_modes: runbook_template_execution_modes(template),
        params: template
            .params
            .iter()
            .map(|param| RunbookTemplateParamItem {
                key: param.key.to_string(),
                label: param.label.to_string(),
                field_type: param.field_type.to_string(),
                required: param.required,
                options: param
                    .options
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect(),
                min_value: param.min_value,
                max_value: param.max_value,
                default_value: param.default_value.map(|item| item.to_string()),
                placeholder: param.placeholder.map(|item| item.to_string()),
            })
            .collect(),
        preflight: template
            .preflight
            .iter()
            .map(|item| RunbookTemplateChecklistItem {
                key: item.key.to_string(),
                label: item.label.to_string(),
                detail: item.detail.to_string(),
            })
            .collect(),
        steps: template
            .steps
            .iter()
            .map(|step| RunbookTemplateStepItem {
                step_id: step.step_id.to_string(),
                name: step.name.to_string(),
                detail: step.detail.to_string(),
            })
            .collect(),
    }
}

fn built_in_runbook_templates() -> Vec<RunbookTemplateDefinition> {
    vec![
        RunbookTemplateDefinition {
            key: "service-restart-safe",
            name: "Service restart (safe)",
            description: "Controlled service restart with preflight ownership and health confirmation.",
            category: "operations",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "asset_ref",
                    label: "Asset reference",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("app-server-01"),
                },
                RunbookTemplateParamDefinition {
                    key: "service_name",
                    label: "Service name",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: Some("nginx"),
                    placeholder: Some("nginx"),
                },
                RunbookTemplateParamDefinition {
                    key: "restart_scope",
                    label: "Restart scope",
                    field_type: "enum",
                    required: true,
                    options: vec!["single-node", "rolling"],
                    min_value: None,
                    max_value: None,
                    default_value: Some("rolling"),
                    placeholder: None,
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("post_health_validation"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_change_window",
                    label: "Change window is confirmed",
                    detail: "Reservation/change window exists and is currently valid.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_owner_ack",
                    label: "Service owner is informed",
                    detail: "Owner/oncall acknowledgment completed before restart.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_rollback_ready",
                    label: "Rollback plan is ready",
                    detail: "Rollback command and health rollback threshold are ready.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "target_validation",
                    name: "Validate target service",
                    detail: "Verify service mapping and restart scope guardrails.",
                    failure_hint: "Verify service name, ownership, and restart scope before retry.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "restart_execution",
                    name: "Execute safe restart",
                    detail: "Run controlled restart command with bounded impact.",
                    failure_hint: "Check process permissions and restart command policy allowlist.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "post_health_validation",
                    name: "Validate post-restart health",
                    detail: "Confirm service endpoint and dependency readiness.",
                    failure_hint: "Run rollback and inspect health probes or dependency saturation before reattempt.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "dependency-check",
            name: "Dependency health check",
            description: "Dependency reachability and readiness verification with remediation context.",
            category: "diagnostics",
            supports_live: true,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "asset_ref",
                    label: "Asset reference",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("app-server-01"),
                },
                RunbookTemplateParamDefinition {
                    key: "dependency_target",
                    label: "Dependency target",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("redis://cache-a:6379"),
                },
                RunbookTemplateParamDefinition {
                    key: "probe_timeout_seconds",
                    label: "Probe timeout (seconds)",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(300),
                    default_value: Some("10"),
                    placeholder: Some("10"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("reachability_probe"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_probe_source",
                    label: "Probe source scope confirmed",
                    detail: "Probe source host/site is approved for diagnostics.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_dependency_owner",
                    label: "Dependency owner contact confirmed",
                    detail: "Owner/escalation route exists if dependency fails.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_ticket_context",
                    label: "Ticket context linked",
                    detail: "Incident/change ticket linked for audit continuity.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "scope_validation",
                    name: "Validate dependency scope",
                    detail: "Confirm dependency target syntax and authorized probing scope.",
                    failure_hint: "Correct target endpoint format and ensure scope authorization.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "reachability_probe",
                    name: "Run dependency probe",
                    detail: "Execute probe and collect response timing.",
                    failure_hint: "Check network ACL/DNS path and retry with dependency owner support.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "readiness_summary",
                    name: "Summarize dependency readiness",
                    detail: "Publish readiness result with latency and error context.",
                    failure_hint: "Capture probe logs and add mitigation note before closure.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "backup-verify",
            name: "Backup verify closeout",
            description: "Backup restore-verification closeout with SLA and evidence linkage.",
            category: "continuity",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "policy_id",
                    label: "Backup policy ID",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: None,
                    default_value: None,
                    placeholder: Some("1"),
                },
                RunbookTemplateParamDefinition {
                    key: "evidence_ticket",
                    label: "Evidence ticket",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("TKT-1234"),
                },
                RunbookTemplateParamDefinition {
                    key: "expected_restore_minutes",
                    label: "Expected restore minutes",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(240),
                    default_value: Some("30"),
                    placeholder: Some("30"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("restore_sla_validation"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_latest_run",
                    label: "Latest backup run selected",
                    detail: "Backup/drill run reference is validated before verification.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_restore_window",
                    label: "Restore window approved",
                    detail: "Restore verification was performed in approved window.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_evidence_artifact",
                    label: "Evidence artifact prepared",
                    detail: "Artifact URL or ticket reference is ready for closure.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "run_lookup",
                    name: "Lookup run context",
                    detail: "Load backup policy and latest run verification context.",
                    failure_hint: "Verify policy/run identifiers and rerun lookup.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "restore_sla_validation",
                    name: "Validate restore SLA",
                    detail: "Compare observed restore signal with expected SLA budget.",
                    failure_hint: "Attach detailed restore metrics and escalate continuity review.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "evidence_closeout",
                    name: "Close evidence record",
                    detail: "Persist evidence summary and close continuity verification.",
                    failure_hint: "Ensure ticket/artifact link exists and retry closeout action.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "maintenance-closeout",
            name: "Maintenance closeout",
            description: "Finalize maintenance execution and publish handover summary.",
            category: "operations",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "change_ticket",
                    label: "Change ticket",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("CHG-20260308-001"),
                },
                RunbookTemplateParamDefinition {
                    key: "change_summary",
                    label: "Change summary",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("patched gateway and rotated certs"),
                },
                RunbookTemplateParamDefinition {
                    key: "signoff_count",
                    label: "Signoff count",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(10),
                    default_value: Some("2"),
                    placeholder: Some("2"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("stakeholder_signoff"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_validation_complete",
                    label: "Validation checks complete",
                    detail: "Post-change validation checklist is complete.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_alerts_stable",
                    label: "Alerts stable",
                    detail: "No unresolved critical alert remains after maintenance.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_handover_ready",
                    label: "Handover summary ready",
                    detail: "Shift handover owner/action are prepared with next steps.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "change_log_collection",
                    name: "Collect change log",
                    detail: "Collect affected services and validation result summary.",
                    failure_hint: "Complete validation checklist and attach missing logs.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "stakeholder_signoff",
                    name: "Confirm stakeholder signoff",
                    detail: "Capture owner/signoff acknowledgements for closure.",
                    failure_hint: "Obtain at least two signoffs or document approved exception before closure.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "handover_publish",
                    name: "Publish handover closeout",
                    detail: "Persist change summary and publish shift handover note.",
                    failure_hint: "Update handover owner/action fields and republish closeout summary.",
                },
            ],
        },
    ]
}

fn normalize_runbook_params(
    template: &RunbookTemplateDefinition,
    raw: Value,
) -> AppResult<JsonMap<String, Value>> {
    let Value::Object(input) = raw else {
        return Err(AppError::Validation(
            "params must be a JSON object".to_string(),
        ));
    };

    let allowed_keys = template
        .params
        .iter()
        .map(|param| param.key)
        .collect::<BTreeSet<_>>();
    for key in input.keys() {
        if !allowed_keys.contains(key.as_str()) {
            return Err(AppError::Validation(format!(
                "unknown runbook parameter '{}'",
                key
            )));
        }
    }

    let mut normalized = JsonMap::new();
    for param in &template.params {
        let value = input.get(param.key).cloned();
        let normalized_value = normalize_single_param(param, value)?;
        if let Some(value) = normalized_value {
            normalized.insert(param.key.to_string(), value);
        }
    }

    Ok(normalized)
}

fn normalize_single_param(
    definition: &RunbookTemplateParamDefinition,
    value: Option<Value>,
) -> AppResult<Option<Value>> {
    let value = match value {
        Some(Value::Null) => None,
        Some(value) => Some(value),
        None => definition
            .default_value
            .map(|default| Value::String(default.to_string())),
    };

    let Some(value) = value else {
        if definition.required {
            return Err(AppError::Validation(format!(
                "parameter '{}' is required",
                definition.key
            )));
        }
        return Ok(None);
    };

    match definition.field_type {
        "string" => {
            let Value::String(raw) = value else {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be a string",
                    definition.key
                )));
            };
            let trimmed = raw.trim();
            if definition.required && trimmed.is_empty() {
                return Err(AppError::Validation(format!(
                    "parameter '{}' cannot be empty",
                    definition.key
                )));
            }
            if trimmed.len() > MAX_TEXT_FIELD_LEN {
                return Err(AppError::Validation(format!(
                    "parameter '{}' length must be <= {}",
                    definition.key, MAX_TEXT_FIELD_LEN
                )));
            }
            Ok(Some(Value::String(trimmed.to_string())))
        }
        "enum" => {
            let Value::String(raw) = value else {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be a string",
                    definition.key
                )));
            };
            let trimmed = raw.trim();
            if definition.required && trimmed.is_empty() {
                return Err(AppError::Validation(format!(
                    "parameter '{}' cannot be empty",
                    definition.key
                )));
            }
            if !definition.options.is_empty()
                && !definition.options.iter().any(|option| *option == trimmed)
            {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be one of: {}",
                    definition.key,
                    definition.options.join(", ")
                )));
            }
            Ok(Some(Value::String(trimmed.to_string())))
        }
        "number" => {
            let number = match value {
                Value::Number(value) => value.as_i64().ok_or_else(|| {
                    AppError::Validation(format!(
                        "parameter '{}' must be an integer",
                        definition.key
                    ))
                })?,
                Value::String(value) => value.trim().parse::<i64>().map_err(|_| {
                    AppError::Validation(format!(
                        "parameter '{}' must be an integer",
                        definition.key
                    ))
                })?,
                _ => {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be a number",
                        definition.key
                    )));
                }
            };

            if let Some(min_value) = definition.min_value {
                if number < min_value {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be >= {}",
                        definition.key, min_value
                    )));
                }
            }
            if let Some(max_value) = definition.max_value {
                if number > max_value {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be <= {}",
                        definition.key, max_value
                    )));
                }
            }
            Ok(Some(Value::Number(number.into())))
        }
        _ => Err(AppError::Validation(format!(
            "unsupported parameter field_type '{}'",
            definition.field_type
        ))),
    }
}

fn normalize_preflight_confirmations(
    template: &RunbookTemplateDefinition,
    raw: Vec<String>,
) -> AppResult<Vec<String>> {
    let mut normalized_set = BTreeSet::new();
    for item in raw {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        normalized_set.insert(trimmed.to_string());
    }

    let mut missing = Vec::new();
    let mut ordered = Vec::new();
    for required in &template.preflight {
        if normalized_set.contains(required.key) {
            ordered.push(required.key.to_string());
        } else {
            missing.push(required.key.to_string());
        }
    }

    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "missing preflight confirmations: {}",
            missing.join(", ")
        )));
    }

    Ok(ordered)
}

fn normalize_runbook_evidence_input(raw: RunbookEvidenceInput) -> AppResult<RunbookEvidenceInput> {
    let summary = required_trimmed("evidence.summary", raw.summary, MAX_NOTE_LEN)?;
    let ticket_ref = trim_optional(raw.ticket_ref, MAX_TICKET_REF_LEN);
    let artifact_url = trim_optional(raw.artifact_url, MAX_ARTIFACT_URL_LEN);

    Ok(RunbookEvidenceInput {
        summary,
        ticket_ref,
        artifact_url,
    })
}

fn evaluate_step_failure(
    template_key: &str,
    step_id: &str,
    params: &JsonMap<String, Value>,
    step_index: usize,
) -> Option<String> {
    if let Some(simulated_step) = string_param(params, "simulate_failure_step") {
        if simulated_step == step_id {
            return Some(format!(
                "simulated failure requested for step '{}'",
                step_id
            ));
        }
    }

    match (template_key, step_id) {
        ("service-restart-safe", "post_health_validation") => {
            if let Some(service_name) = string_param(params, "service_name") {
                if service_name.contains("legacy") {
                    return Some(
                        "health probe failed: legacy service profile requires manual warmup"
                            .to_string(),
                    );
                }
            }
        }
        ("dependency-check", "reachability_probe") => {
            if let Some(target) = string_param(params, "dependency_target") {
                if target.contains("unstable") {
                    return Some(
                        "dependency probe timeout exceeded due to unstable endpoint".to_string(),
                    );
                }
            }
        }
        ("backup-verify", "restore_sla_validation") => {
            if let Some(minutes) = number_param(params, "expected_restore_minutes") {
                if minutes > 30 {
                    return Some(format!(
                        "restore SLA validation failed: expected_restore_minutes={} exceeds budget",
                        minutes
                    ));
                }
            }
        }
        ("maintenance-closeout", "stakeholder_signoff") => {
            if let Some(count) = number_param(params, "signoff_count") {
                if count < 2 {
                    return Some(format!(
                        "stakeholder signoff requirement not met: signoff_count={}",
                        count
                    ));
                }
            }
        }
        _ => {}
    }

    if step_index > 8 {
        return Some("guardrail abort: unexpected step depth".to_string());
    }

    None
}

fn string_param(params: &JsonMap<String, Value>, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn number_param(params: &JsonMap<String, Value>, key: &str) -> Option<i64> {
    params.get(key).and_then(|value| value.as_i64())
}

fn normalize_template_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("template key is required".to_string()));
    }
    if normalized.len() > MAX_TEMPLATE_KEY_LEN {
        return Err(AppError::Validation(format!(
            "template key length must be <= {}",
            MAX_TEMPLATE_KEY_LEN
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(AppError::Validation(
            "template key must only contain lowercase letters, digits, or '-'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_execution_status(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "succeeded" | "failed" => Ok(normalized),
        _ => Err(AppError::Validation(
            "status must be one of: succeeded, failed".to_string(),
        )),
    }
}

fn required_trimmed(field: &str, value: String, max_len: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if trimmed.len() > max_len {
        return Err(AppError::Validation(format!(
            "{field} length must be <= {max_len}"
        )));
    }
    Ok(trimmed.to_string())
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.chars().take(max_len).collect())
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        enforce_template_supports_execution_mode, evaluate_step_failure, normalize_execution_mode,
        normalize_live_template_keys, normalize_preflight_confirmations, normalize_preset_name,
        normalize_runbook_params, parse_dependency_target, parse_preflight_snapshot_confirmed,
        resolve_template_by_key,
    };

    #[test]
    fn validates_runbook_param_guardrails() {
        let template = resolve_template_by_key("dependency-check").expect("template");
        let normalized = normalize_runbook_params(
            &template,
            json!({
                "asset_ref": "app-1",
                "dependency_target": "redis://cache-a:6379",
                "probe_timeout_seconds": 20
            }),
        )
        .expect("normalized");
        assert_eq!(
            normalized
                .get("probe_timeout_seconds")
                .and_then(|value| value.as_i64()),
            Some(20)
        );

        let err = normalize_runbook_params(
            &template,
            json!({
                "asset_ref": "app-1",
                "dependency_target": "redis://cache-a:6379",
                "probe_timeout_seconds": 500
            }),
        )
        .expect_err("out of range timeout should fail");
        assert!(format!("{}", err).contains("must be <= 300"));
    }

    #[test]
    fn requires_all_preflight_confirmations() {
        let template = resolve_template_by_key("service-restart-safe").expect("template");
        let err =
            normalize_preflight_confirmations(&template, vec!["confirm_change_window".to_string()])
                .expect_err("missing confirmations should fail");
        assert!(format!("{}", err).contains("missing preflight confirmations"));
    }

    #[test]
    fn provides_remediation_hint_for_failed_step() {
        let template = resolve_template_by_key("maintenance-closeout").expect("template");
        let params = normalize_runbook_params(
            &template,
            json!({
                "change_ticket": "CHG-001",
                "change_summary": "patched",
                "signoff_count": 1
            }),
        )
        .expect("params");

        let failure = evaluate_step_failure(template.key, "stakeholder_signoff", &params, 1);
        assert!(failure.is_some());
    }

    #[test]
    fn normalizes_live_template_allowlist() {
        let templates = normalize_live_template_keys(vec![
            "dependency-check".to_string(),
            "dependency-check".to_string(),
        ])
        .expect("live templates");
        assert_eq!(templates, vec!["dependency-check".to_string()]);

        let err = normalize_live_template_keys(vec!["backup-verify".to_string()])
            .expect_err("non-live template should fail");
        assert!(format!("{}", err).contains("does not support live execution"));
    }

    #[test]
    fn parses_dependency_target_with_scheme_and_port() {
        let target = parse_dependency_target("http://127.0.0.1:8080/health").expect("valid target");
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 8080);

        let target = parse_dependency_target("https://example.local/api").expect("https target");
        assert_eq!(target.host, "example.local");
        assert_eq!(target.port, 443);

        let target = parse_dependency_target("redis.local:6379").expect("host:port target");
        assert_eq!(target.host, "redis.local");
        assert_eq!(target.port, 6379);
    }

    #[test]
    fn validates_execution_mode_values() {
        assert_eq!(
            normalize_execution_mode("simulate".to_string()).expect("simulate"),
            "simulate"
        );
        assert_eq!(
            normalize_execution_mode("live".to_string()).expect("live"),
            "live"
        );

        let err = normalize_execution_mode("invalid".to_string()).expect_err("should fail");
        assert!(format!("{}", err).contains("execution_mode must be one of"));
    }

    #[test]
    fn validates_preset_name_rules() {
        assert_eq!(
            normalize_preset_name("  dependency baseline  ".to_string()).expect("preset name"),
            "dependency baseline"
        );
        assert!(normalize_preset_name("".to_string()).is_err());
        assert!(normalize_preset_name("x".repeat(129)).is_err());
    }

    #[test]
    fn validates_template_execution_mode_support() {
        let live_template = resolve_template_by_key("dependency-check").expect("template");
        enforce_template_supports_execution_mode(&live_template, "live")
            .expect("live template supports live mode");

        let simulate_only_template = resolve_template_by_key("backup-verify").expect("template");
        let err = enforce_template_supports_execution_mode(&simulate_only_template, "live")
            .expect_err("should reject live mode");
        assert!(format!("{}", err).contains("does not support live execution"));
    }

    #[test]
    fn parses_preflight_snapshot_confirmed_values() {
        let confirmed = parse_preflight_snapshot_confirmed(json!({
            "confirmed": ["confirm_probe_source", "confirm_dependency_owner"],
            "total_required": 3
        }))
        .expect("confirmed list");
        assert_eq!(confirmed.len(), 2);
        assert_eq!(confirmed[0], "confirm_probe_source");
        assert_eq!(confirmed[1], "confirm_dependency_owner");

        let err = parse_preflight_snapshot_confirmed(json!({
            "total_required": 3
        }))
        .expect_err("missing confirmed should fail");
        assert!(format!("{}", err).contains("missing confirmed"));
    }
}
