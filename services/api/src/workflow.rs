use std::path::Path as FsPath;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, QueryBuilder};
use tokio::{
    process::Command,
    time::{Duration, timeout},
};

use crate::{
    audit::{actor_from_headers, write_from_headers_best_effort},
    error::{AppError, AppResult},
    state::{AppState, WorkflowExecutionSettings},
};

const MAX_TEMPLATE_NAME_LEN: usize = 128;
const MAX_STEP_ID_LEN: usize = 64;
const MAX_STEP_NAME_LEN: usize = 128;
const MAX_STEP_SCRIPT_LEN: usize = 16_000;
const MAX_REQUEST_TITLE_LEN: usize = 255;
const MAX_REASON_LEN: usize = 1_024;
const MAX_LOG_OUTPUT_LEN: usize = 16_000;
const DEFAULT_SCRIPT_TIMEOUT_SECONDS: u64 = 300;
const MAX_SCRIPT_TIMEOUT_SECONDS: u64 = 3_600;
const MAX_COMMAND_LINE_LEN: usize = 4_096;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/templates", get(list_templates).post(create_template))
        .route("/requests", get(list_requests).post(create_request))
        .route("/requests/{id}", get(get_request))
        .route("/requests/{id}/logs", get(list_request_logs))
        .route(
            "/requests/{id}/execute",
            axum::routing::post(execute_request),
        )
        .route(
            "/requests/{id}/manual-complete",
            axum::routing::post(complete_manual_step),
        )
        .route(
            "/approvals/{id}/approve",
            axum::routing::post(approve_request),
        )
        .route(
            "/approvals/{id}/reject",
            axum::routing::post(reject_request),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WorkflowTemplate {
    id: i64,
    name: String,
    description: Option<String>,
    definition: Value,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WorkflowRequest {
    id: i64,
    template_id: i64,
    template_name: String,
    title: String,
    requester: String,
    status: String,
    current_step_index: i32,
    payload: Value,
    last_error: Option<String>,
    approved_by: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    executed_by: Option<String>,
    executed_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct WorkflowExecutionLog {
    id: i64,
    request_id: i64,
    step_index: i32,
    step_id: String,
    step_name: String,
    step_kind: String,
    status: String,
    executor: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    duration_ms: Option<i32>,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct RequestExecutionContext {
    status: String,
    current_step_index: i32,
    definition: Value,
}

#[derive(Debug, Deserialize)]
struct CreateWorkflowTemplateRequest {
    name: String,
    description: Option<String>,
    definition: Value,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateWorkflowRequestRequest {
    template_id: i64,
    title: String,
    payload: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RejectWorkflowRequestRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManualCompleteRequest {
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListWorkflowTemplatesQuery {
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListWorkflowRequestsQuery {
    status: Option<String>,
    template_id: Option<i64>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct WorkflowDefinitionInput {
    steps: Vec<WorkflowStepInput>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStepInput {
    id: String,
    name: String,
    kind: String,
    auto_run: Option<bool>,
    script: Option<String>,
    timeout_seconds: Option<u64>,
    approver_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowDefinition {
    steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowStep {
    id: String,
    name: String,
    kind: WorkflowStepKind,
    auto_run: bool,
    script: Option<String>,
    timeout_seconds: u64,
    approver_group: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkflowStepKind {
    Approval,
    Script,
    Manual,
}

#[derive(Debug)]
struct ScriptExecutionResult {
    success: bool,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
    duration_ms: i32,
    policy_mode: String,
    policy_decision: String,
    command_hash_sha256: String,
    command: Option<String>,
    allowlist_match: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptExecutionPolicyMode {
    Disabled,
    Allowlist,
    Sandboxed,
}

impl ScriptExecutionPolicyMode {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "disabled" => Some(Self::Disabled),
            "allowlist" => Some(Self::Allowlist),
            "sandboxed" => Some(Self::Sandboxed),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Allowlist => "allowlist",
            Self::Sandboxed => "sandboxed",
        }
    }
}

#[derive(Debug, Clone)]
struct ScriptExecutionPolicy {
    mode: ScriptExecutionPolicyMode,
    allowlist: Vec<String>,
    sandbox_dir: String,
}

#[derive(Debug, Clone)]
struct ScriptCommandSpec {
    executable: String,
    args: Vec<String>,
}

#[derive(Debug)]
struct ExecutionLogInput {
    request_id: i64,
    step_index: i32,
    step_id: String,
    step_name: String,
    step_kind: String,
    status: String,
    executor: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    duration_ms: Option<i32>,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
    metadata: Value,
}

async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkflowTemplateRequest>,
) -> AppResult<Json<WorkflowTemplate>> {
    let name = required_trimmed("name", payload.name, MAX_TEMPLATE_NAME_LEN)?;
    let description = trim_optional(payload.description, 2_048);
    let definition = normalize_workflow_definition(payload.definition)?;
    let definition_value = serde_json::to_value(definition)
        .map_err(|err| AppError::Validation(format!("definition serialization failed: {err}")))?;
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let item: WorkflowTemplate = sqlx::query_as(
        "INSERT INTO workflow_templates (name, description, definition_json, is_enabled)
         VALUES ($1, $2, $3, $4)
         RETURNING id, name, description, definition_json AS definition, is_enabled, created_at, updated_at",
    )
    .bind(&name)
    .bind(description)
    .bind(definition_value)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_template_unique_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.template.create",
        "workflow_template",
        Some(item.id.to_string()),
        "success",
        None,
        json!({ "name": item.name }),
    )
    .await;

    Ok(Json(item))
}

async fn list_templates(
    State(state): State<AppState>,
    Query(query): Query<ListWorkflowTemplatesQuery>,
) -> AppResult<Json<Vec<WorkflowTemplate>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, name, description, definition_json AS definition, is_enabled, created_at, updated_at
         FROM workflow_templates
         WHERE 1=1",
    );

    if let Some(is_enabled) = query.is_enabled {
        builder.push(" AND is_enabled = ").push_bind(is_enabled);
    }

    builder.push(" ORDER BY id DESC");

    let items: Vec<WorkflowTemplate> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn create_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateWorkflowRequestRequest>,
) -> AppResult<Json<WorkflowRequest>> {
    let title = required_trimmed("title", payload.title, MAX_REQUEST_TITLE_LEN)?;
    let requester = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());
    let workflow_payload = normalize_request_payload(payload.payload)?;

    let template_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM workflow_templates
            WHERE id = $1
              AND is_enabled = TRUE
        )",
    )
    .bind(payload.template_id)
    .fetch_one(&state.db)
    .await?;

    if !template_exists {
        return Err(AppError::Validation(format!(
            "workflow template {} does not exist or is disabled",
            payload.template_id
        )));
    }

    let request_id: i64 = sqlx::query_scalar(
        "INSERT INTO workflow_requests (template_id, title, requester, status, payload)
         VALUES ($1, $2, $3, 'pending_approval', $4)
         RETURNING id",
    )
    .bind(payload.template_id)
    .bind(&title)
    .bind(&requester)
    .bind(workflow_payload)
    .fetch_one(&state.db)
    .await?;

    let item = fetch_request_by_id(&state.db, request_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.request.create",
        "workflow_request",
        Some(item.id.to_string()),
        "success",
        None,
        json!({
            "template_id": item.template_id,
            "title": item.title
        }),
    )
    .await;

    Ok(Json(item))
}

async fn list_requests(
    State(state): State<AppState>,
    Query(query): Query<ListWorkflowRequestsQuery>,
) -> AppResult<Json<Vec<WorkflowRequest>>> {
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT r.id,
                r.template_id,
                t.name AS template_name,
                r.title,
                r.requester,
                r.status,
                r.current_step_index,
                r.payload,
                r.last_error,
                r.approved_by,
                r.approved_at,
                r.executed_by,
                r.executed_at,
                r.completed_at,
                r.created_at,
                r.updated_at
         FROM workflow_requests r
         INNER JOIN workflow_templates t ON t.id = r.template_id
         WHERE 1=1",
    );

    if let Some(status) = trim_optional(query.status, 32) {
        builder.push(" AND r.status = ").push_bind(status);
    }
    if let Some(template_id) = query.template_id {
        builder.push(" AND r.template_id = ").push_bind(template_id);
    }

    builder
        .push(" ORDER BY r.created_at DESC, r.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<WorkflowRequest> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn get_request(
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
) -> AppResult<Json<WorkflowRequest>> {
    let item = fetch_request_by_id(&state.db, request_id).await?;
    Ok(Json(item))
}

async fn approve_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<i64>,
) -> AppResult<Json<WorkflowRequest>> {
    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());

    let updated: Option<i64> = sqlx::query_scalar(
        "UPDATE workflow_requests
         SET status = 'approved',
             approved_by = $2,
             approved_at = NOW(),
             updated_at = NOW(),
             last_error = NULL
         WHERE id = $1
           AND status = 'pending_approval'
         RETURNING id",
    )
    .bind(request_id)
    .bind(&actor)
    .fetch_optional(&state.db)
    .await?;

    if updated.is_none() {
        return Err(AppError::Validation(
            "request must be in pending_approval status to approve".to_string(),
        ));
    }

    let item = fetch_request_by_id(&state.db, request_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.request.approve",
        "workflow_request",
        Some(request_id.to_string()),
        "success",
        None,
        json!({ "status": item.status }),
    )
    .await;

    Ok(Json(item))
}

async fn reject_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<i64>,
    Json(payload): Json<RejectWorkflowRequestRequest>,
) -> AppResult<Json<WorkflowRequest>> {
    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());
    let reason = trim_optional(payload.reason, MAX_REASON_LEN)
        .unwrap_or_else(|| "rejected by approver".to_string());

    let updated: Option<i64> = sqlx::query_scalar(
        "UPDATE workflow_requests
         SET status = 'rejected',
             approved_by = $2,
             approved_at = NOW(),
             updated_at = NOW(),
             last_error = $3
         WHERE id = $1
           AND status = 'pending_approval'
         RETURNING id",
    )
    .bind(request_id)
    .bind(&actor)
    .bind(&reason)
    .fetch_optional(&state.db)
    .await?;

    if updated.is_none() {
        return Err(AppError::Validation(
            "request must be in pending_approval status to reject".to_string(),
        ));
    }

    let item = fetch_request_by_id(&state.db, request_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.request.reject",
        "workflow_request",
        Some(request_id.to_string()),
        "success",
        Some(reason),
        json!({ "status": item.status }),
    )
    .await;

    Ok(Json(item))
}

async fn execute_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<i64>,
) -> AppResult<Json<WorkflowRequest>> {
    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());

    let context = fetch_execution_context(&state.db, request_id).await?;
    if !matches!(
        context.status.as_str(),
        "approved" | "running" | "waiting_manual"
    ) {
        return Err(AppError::Validation(
            "request must be approved/running/waiting_manual to execute".to_string(),
        ));
    }

    if context.status == "waiting_manual" {
        return Err(AppError::Validation(
            "request is waiting on manual step; use manual-complete first".to_string(),
        ));
    }

    let definition = parse_workflow_definition(context.definition)?;

    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'running',
             executed_by = COALESCE(executed_by, $2),
             executed_at = COALESCE(executed_at, NOW()),
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(&actor)
    .execute(&state.db)
    .await?;

    execute_from_step(
        &state,
        request_id,
        context.current_step_index.max(0) as usize,
        &definition,
        &actor,
    )
    .await?;

    let item = fetch_request_by_id(&state.db, request_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.request.execute",
        "workflow_request",
        Some(request_id.to_string()),
        "success",
        None,
        json!({
            "status": item.status,
            "execution_policy_mode": state.workflow_execution.policy_mode
        }),
    )
    .await;

    Ok(Json(item))
}

async fn complete_manual_step(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request_id): Path<i64>,
    Json(payload): Json<ManualCompleteRequest>,
) -> AppResult<Json<WorkflowRequest>> {
    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());

    let context = fetch_execution_context(&state.db, request_id).await?;
    if context.status != "waiting_manual" {
        return Err(AppError::Validation(
            "request is not waiting for a manual step".to_string(),
        ));
    }

    let definition = parse_workflow_definition(context.definition)?;
    let current_index = context.current_step_index.max(0) as usize;
    let Some(current_step) = definition.steps.get(current_index) else {
        return Err(AppError::Validation(
            "current step index is out of range".to_string(),
        ));
    };

    let is_manual_barrier = current_step.kind == WorkflowStepKind::Manual
        || (current_step.kind == WorkflowStepKind::Script && !current_step.auto_run);

    if !is_manual_barrier {
        return Err(AppError::Validation(
            "current step is not a manual barrier".to_string(),
        ));
    }

    let note = trim_optional(payload.note, MAX_REASON_LEN);
    insert_execution_log(
        &state.db,
        ExecutionLogInput {
            request_id,
            step_index: current_index as i32,
            step_id: current_step.id.clone(),
            step_name: current_step.name.clone(),
            step_kind: workflow_step_kind_label(current_step.kind).to_string(),
            status: "success".to_string(),
            executor: Some(actor.clone()),
            started_at: Some(Utc::now()),
            finished_at: Some(Utc::now()),
            duration_ms: Some(0),
            exit_code: None,
            output: note.clone(),
            error: None,
            metadata: json!({
                "manual": true,
                "auto_run": current_step.auto_run
            }),
        },
    )
    .await?;

    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'running',
             current_step_index = $2,
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind((current_index + 1) as i32)
    .execute(&state.db)
    .await?;

    execute_from_step(&state, request_id, current_index + 1, &definition, &actor).await?;

    let item = fetch_request_by_id(&state.db, request_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.request.manual_complete",
        "workflow_request",
        Some(request_id.to_string()),
        "success",
        note,
        json!({ "status": item.status }),
    )
    .await;

    Ok(Json(item))
}

async fn list_request_logs(
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
) -> AppResult<Json<Vec<WorkflowExecutionLog>>> {
    let request_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_requests WHERE id = $1)")
            .bind(request_id)
            .fetch_one(&state.db)
            .await?;

    if !request_exists {
        return Err(AppError::NotFound(format!(
            "workflow request {request_id} not found"
        )));
    }

    let items: Vec<WorkflowExecutionLog> = sqlx::query_as(
        "SELECT id,
                request_id,
                step_index,
                step_id,
                step_name,
                step_kind,
                status,
                executor,
                started_at,
                finished_at,
                duration_ms,
                exit_code,
                output,
                error,
                metadata,
                created_at,
                updated_at
         FROM workflow_execution_logs
         WHERE request_id = $1
         ORDER BY id ASC",
    )
    .bind(request_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(items))
}

async fn execute_from_step(
    state: &AppState,
    request_id: i64,
    start_index: usize,
    definition: &WorkflowDefinition,
    actor: &str,
) -> AppResult<()> {
    let mut index = start_index;
    let script_policy = script_execution_policy_from_settings(&state.workflow_execution);

    while index < definition.steps.len() {
        let step = &definition.steps[index];

        match step.kind {
            WorkflowStepKind::Approval => {
                insert_execution_log(
                    &state.db,
                    ExecutionLogInput {
                        request_id,
                        step_index: index as i32,
                        step_id: step.id.clone(),
                        step_name: step.name.clone(),
                        step_kind: workflow_step_kind_label(step.kind).to_string(),
                        status: "skipped".to_string(),
                        executor: Some(actor.to_string()),
                        started_at: Some(Utc::now()),
                        finished_at: Some(Utc::now()),
                        duration_ms: Some(0),
                        exit_code: None,
                        output: Some("approval already completed".to_string()),
                        error: None,
                        metadata: json!({
                            "kind": "approval"
                        }),
                    },
                )
                .await?;

                index += 1;
                update_request_progress(&state.db, request_id, index as i32).await?;
            }
            WorkflowStepKind::Manual => {
                insert_execution_log(
                    &state.db,
                    ExecutionLogInput {
                        request_id,
                        step_index: index as i32,
                        step_id: step.id.clone(),
                        step_name: step.name.clone(),
                        step_kind: workflow_step_kind_label(step.kind).to_string(),
                        status: "manual_wait".to_string(),
                        executor: Some(actor.to_string()),
                        started_at: Some(Utc::now()),
                        finished_at: Some(Utc::now()),
                        duration_ms: Some(0),
                        exit_code: None,
                        output: Some("manual step requires operator input".to_string()),
                        error: None,
                        metadata: json!({
                            "auto_run": step.auto_run
                        }),
                    },
                )
                .await?;

                update_request_waiting_manual(&state.db, request_id, index as i32).await?;
                return Ok(());
            }
            WorkflowStepKind::Script => {
                if !step.auto_run {
                    insert_execution_log(
                        &state.db,
                        ExecutionLogInput {
                            request_id,
                            step_index: index as i32,
                            step_id: step.id.clone(),
                            step_name: step.name.clone(),
                            step_kind: workflow_step_kind_label(step.kind).to_string(),
                            status: "manual_wait".to_string(),
                            executor: Some(actor.to_string()),
                            started_at: Some(Utc::now()),
                            finished_at: Some(Utc::now()),
                            duration_ms: Some(0),
                            exit_code: None,
                            output: Some("script step set to manual execution".to_string()),
                            error: None,
                            metadata: json!({
                                "auto_run": false
                            }),
                        },
                    )
                    .await?;

                    update_request_waiting_manual(&state.db, request_id, index as i32).await?;
                    return Ok(());
                }

                let started_at = Utc::now();
                let script = step.script.clone().unwrap_or_default();
                let timeout_seconds = step.timeout_seconds;
                let script_result = run_shell_script(script, timeout_seconds, &script_policy).await;
                let finished_at = Utc::now();

                if script_result.success {
                    insert_execution_log(
                        &state.db,
                        ExecutionLogInput {
                            request_id,
                            step_index: index as i32,
                            step_id: step.id.clone(),
                            step_name: step.name.clone(),
                            step_kind: workflow_step_kind_label(step.kind).to_string(),
                            status: "success".to_string(),
                            executor: Some(actor.to_string()),
                            started_at: Some(started_at),
                            finished_at: Some(finished_at),
                            duration_ms: Some(script_result.duration_ms),
                            exit_code: script_result.exit_code,
                            output: script_result.output,
                            error: None,
                            metadata: json!({
                                "timeout_seconds": timeout_seconds,
                                "execution_policy": {
                                    "mode": script_result.policy_mode,
                                    "decision": script_result.policy_decision,
                                    "command": script_result.command,
                                    "command_hash_sha256": script_result.command_hash_sha256,
                                    "allowlist_match": script_result.allowlist_match
                                }
                            }),
                        },
                    )
                    .await?;

                    index += 1;
                    update_request_progress(&state.db, request_id, index as i32).await?;
                    continue;
                }

                let failure_message = script_result
                    .error
                    .clone()
                    .unwrap_or_else(|| "script execution failed".to_string());

                insert_execution_log(
                    &state.db,
                    ExecutionLogInput {
                        request_id,
                        step_index: index as i32,
                        step_id: step.id.clone(),
                        step_name: step.name.clone(),
                        step_kind: workflow_step_kind_label(step.kind).to_string(),
                        status: "failed".to_string(),
                        executor: Some(actor.to_string()),
                        started_at: Some(started_at),
                        finished_at: Some(finished_at),
                        duration_ms: Some(script_result.duration_ms),
                        exit_code: script_result.exit_code,
                        output: script_result.output,
                        error: Some(failure_message.clone()),
                        metadata: json!({
                            "timeout_seconds": timeout_seconds,
                            "execution_policy": {
                                "mode": script_result.policy_mode,
                                "decision": script_result.policy_decision,
                                "command": script_result.command,
                                "command_hash_sha256": script_result.command_hash_sha256,
                                "allowlist_match": script_result.allowlist_match
                            }
                        }),
                    },
                )
                .await?;

                update_request_failed(&state.db, request_id, index as i32, failure_message).await?;
                return Ok(());
            }
        }
    }

    update_request_completed(&state.db, request_id, definition.steps.len() as i32).await?;
    Ok(())
}

async fn run_shell_script(
    script: String,
    timeout_seconds: u64,
    policy: &ScriptExecutionPolicy,
) -> ScriptExecutionResult {
    let timeout_seconds = timeout_seconds.clamp(1, MAX_SCRIPT_TIMEOUT_SECONDS);
    let command_hash_sha256 = hash_text_sha256(&script);

    let command_spec = match parse_script_command(script.as_str()) {
        Ok(spec) => spec,
        Err(err) => {
            return ScriptExecutionResult {
                success: false,
                exit_code: None,
                output: None,
                error: Some(err.to_string()),
                duration_ms: 0,
                policy_mode: policy.mode.as_str().to_string(),
                policy_decision: "blocked_parse_error".to_string(),
                command_hash_sha256,
                command: None,
                allowlist_match: None,
            };
        }
    };

    if policy.mode == ScriptExecutionPolicyMode::Disabled {
        return ScriptExecutionResult {
            success: false,
            exit_code: None,
            output: None,
            error: Some(
                "workflow script execution is disabled by WORKFLOW_EXECUTION_POLICY_MODE=disabled"
                    .to_string(),
            ),
            duration_ms: 0,
            policy_mode: policy.mode.as_str().to_string(),
            policy_decision: "blocked_policy_disabled".to_string(),
            command_hash_sha256,
            command: Some(command_spec.executable),
            allowlist_match: None,
        };
    }

    let allowlist_match =
        match match_allowlist_command(command_spec.executable.as_str(), &policy.allowlist) {
            Some(matched) => matched,
            None => {
                return ScriptExecutionResult {
                    success: false,
                    exit_code: None,
                    output: None,
                    error: Some(format!(
                        "command '{}' is not in WORKFLOW_EXECUTION_ALLOWLIST",
                        command_spec.executable
                    )),
                    duration_ms: 0,
                    policy_mode: policy.mode.as_str().to_string(),
                    policy_decision: "blocked_not_allowlisted".to_string(),
                    command_hash_sha256,
                    command: Some(command_spec.executable),
                    allowlist_match: None,
                };
            }
        };

    let mut command = Command::new(&command_spec.executable);
    command.args(&command_spec.args);
    command.kill_on_drop(true);

    if policy.mode == ScriptExecutionPolicyMode::Sandboxed {
        if let Err(err) = std::fs::create_dir_all(&policy.sandbox_dir) {
            return ScriptExecutionResult {
                success: false,
                exit_code: None,
                output: None,
                error: Some(format!(
                    "failed to prepare workflow sandbox directory '{}': {}",
                    policy.sandbox_dir, err
                )),
                duration_ms: 0,
                policy_mode: policy.mode.as_str().to_string(),
                policy_decision: "blocked_sandbox_init_failed".to_string(),
                command_hash_sha256,
                command: Some(command_spec.executable),
                allowlist_match: Some(allowlist_match),
            };
        }

        command.current_dir(&policy.sandbox_dir);
        command.env_clear();
        command.env("PATH", "/usr/bin:/bin");
        command.env("HOME", &policy.sandbox_dir);
    }

    let started = std::time::Instant::now();
    let output_result = timeout(Duration::from_secs(timeout_seconds), command.output()).await;
    let policy_mode = policy.mode.as_str().to_string();
    let command = Some(command_spec.executable);
    let allowlist_match = Some(allowlist_match);

    match output_result {
        Ok(Ok(output)) => {
            let duration_ms = elapsed_to_i32(started.elapsed().as_millis());
            let exit_code = output.status.code();
            let merged_output = merge_stdout_stderr(&output.stdout, &output.stderr);
            if output.status.success() {
                ScriptExecutionResult {
                    success: true,
                    exit_code,
                    output: Some(truncate_text(merged_output, MAX_LOG_OUTPUT_LEN)),
                    error: None,
                    duration_ms,
                    policy_mode,
                    policy_decision: "allowed".to_string(),
                    command_hash_sha256,
                    command,
                    allowlist_match,
                }
            } else {
                ScriptExecutionResult {
                    success: false,
                    exit_code,
                    output: Some(truncate_text(merged_output, MAX_LOG_OUTPUT_LEN)),
                    error: Some(format!(
                        "command exited with code {}",
                        exit_code.unwrap_or(-1)
                    )),
                    duration_ms,
                    policy_mode,
                    policy_decision: "allowed_runtime_failed".to_string(),
                    command_hash_sha256,
                    command,
                    allowlist_match,
                }
            }
        }
        Ok(Err(err)) => ScriptExecutionResult {
            success: false,
            exit_code: None,
            output: None,
            error: Some(format!("command launch failed: {err}")),
            duration_ms: elapsed_to_i32(started.elapsed().as_millis()),
            policy_mode,
            policy_decision: "allowed_launch_failed".to_string(),
            command_hash_sha256,
            command,
            allowlist_match,
        },
        Err(_) => ScriptExecutionResult {
            success: false,
            exit_code: None,
            output: None,
            error: Some(format!(
                "command timed out after {} seconds",
                timeout_seconds
            )),
            duration_ms: elapsed_to_i32(started.elapsed().as_millis()),
            policy_mode,
            policy_decision: "allowed_timeout".to_string(),
            command_hash_sha256,
            command,
            allowlist_match,
        },
    }
}

fn script_execution_policy_from_settings(
    settings: &WorkflowExecutionSettings,
) -> ScriptExecutionPolicy {
    let mode = ScriptExecutionPolicyMode::from_str(settings.policy_mode.as_str())
        .unwrap_or(ScriptExecutionPolicyMode::Disabled);
    let allowlist = settings
        .allowlist
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect();

    ScriptExecutionPolicy {
        mode,
        allowlist,
        sandbox_dir: settings.sandbox_dir.clone(),
    }
}

fn parse_script_command(script: &str) -> AppResult<ScriptCommandSpec> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "script command cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > MAX_COMMAND_LINE_LEN {
        return Err(AppError::Validation(format!(
            "script command exceeds max length {MAX_COMMAND_LINE_LEN}"
        )));
    }

    let tokens: Vec<String> = if trimmed.starts_with('[') {
        let parsed: Vec<String> = serde_json::from_str(trimmed).map_err(|err| {
            AppError::Validation(format!("script command JSON array is invalid: {err}"))
        })?;
        if parsed.is_empty() {
            return Err(AppError::Validation(
                "script command JSON array cannot be empty".to_string(),
            ));
        }
        parsed
    } else {
        shell_words::split(trimmed)
            .map_err(|err| AppError::Validation(format!("script command parsing failed: {err}")))?
    };

    if tokens.is_empty() {
        return Err(AppError::Validation(
            "script command cannot be empty".to_string(),
        ));
    }

    let executable = tokens[0].trim().to_string();
    if executable.is_empty() {
        return Err(AppError::Validation(
            "script command executable cannot be empty".to_string(),
        ));
    }

    let args = tokens
        .into_iter()
        .skip(1)
        .map(|item| item.trim().to_string())
        .collect();

    Ok(ScriptCommandSpec { executable, args })
}

fn match_allowlist_command(executable: &str, allowlist: &[String]) -> Option<String> {
    let executable_trimmed = executable.trim();
    if executable_trimmed.is_empty() {
        return None;
    }

    let executable_base = command_basename(executable_trimmed);
    for item in allowlist {
        let candidate = item.trim();
        if candidate.is_empty() {
            continue;
        }
        if candidate == executable_trimmed {
            return Some(candidate.to_string());
        }
        if command_basename(candidate) == executable_base {
            return Some(candidate.to_string());
        }
    }

    None
}

fn command_basename(value: &str) -> &str {
    FsPath::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
}

fn hash_text_sha256(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn insert_execution_log(db: &sqlx::PgPool, input: ExecutionLogInput) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO workflow_execution_logs (
            request_id,
            step_index,
            step_id,
            step_name,
            step_kind,
            status,
            executor,
            started_at,
            finished_at,
            duration_ms,
            exit_code,
            output,
            error,
            metadata
         ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10, $11, $12, $13, $14
         )",
    )
    .bind(input.request_id)
    .bind(input.step_index)
    .bind(input.step_id)
    .bind(input.step_name)
    .bind(input.step_kind)
    .bind(input.status)
    .bind(input.executor)
    .bind(input.started_at)
    .bind(input.finished_at)
    .bind(input.duration_ms)
    .bind(input.exit_code)
    .bind(input.output)
    .bind(input.error)
    .bind(input.metadata)
    .execute(db)
    .await?;

    Ok(())
}

async fn update_request_progress(
    db: &sqlx::PgPool,
    request_id: i64,
    current_step_index: i32,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'running',
             current_step_index = $2,
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(current_step_index)
    .execute(db)
    .await?;

    Ok(())
}

async fn update_request_waiting_manual(
    db: &sqlx::PgPool,
    request_id: i64,
    current_step_index: i32,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'waiting_manual',
             current_step_index = $2,
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(current_step_index)
    .execute(db)
    .await?;

    Ok(())
}

async fn update_request_failed(
    db: &sqlx::PgPool,
    request_id: i64,
    current_step_index: i32,
    reason: String,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'failed',
             current_step_index = $2,
             last_error = $3,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(current_step_index)
    .bind(reason)
    .execute(db)
    .await?;

    Ok(())
}

async fn update_request_completed(
    db: &sqlx::PgPool,
    request_id: i64,
    current_step_index: i32,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE workflow_requests
         SET status = 'completed',
             current_step_index = $2,
             completed_at = NOW(),
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(request_id)
    .bind(current_step_index)
    .execute(db)
    .await?;

    Ok(())
}

async fn fetch_request_by_id(db: &sqlx::PgPool, request_id: i64) -> AppResult<WorkflowRequest> {
    let item: Option<WorkflowRequest> = sqlx::query_as(
        "SELECT r.id,
                r.template_id,
                t.name AS template_name,
                r.title,
                r.requester,
                r.status,
                r.current_step_index,
                r.payload,
                r.last_error,
                r.approved_by,
                r.approved_at,
                r.executed_by,
                r.executed_at,
                r.completed_at,
                r.created_at,
                r.updated_at
         FROM workflow_requests r
         INNER JOIN workflow_templates t ON t.id = r.template_id
         WHERE r.id = $1",
    )
    .bind(request_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("workflow request {request_id} not found")))
}

async fn fetch_execution_context(
    db: &sqlx::PgPool,
    request_id: i64,
) -> AppResult<RequestExecutionContext> {
    let item: Option<RequestExecutionContext> = sqlx::query_as(
        "SELECT r.status,
                r.current_step_index,
                t.definition_json AS definition
         FROM workflow_requests r
         INNER JOIN workflow_templates t ON t.id = r.template_id
         WHERE r.id = $1",
    )
    .bind(request_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("workflow request {request_id} not found")))
}

fn normalize_workflow_definition(definition: Value) -> AppResult<WorkflowDefinition> {
    let parsed: WorkflowDefinitionInput = serde_json::from_value(definition)
        .map_err(|err| AppError::Validation(format!("invalid definition JSON: {err}")))?;

    if parsed.steps.is_empty() {
        return Err(AppError::Validation(
            "workflow definition must contain at least one step".to_string(),
        ));
    }

    if parsed.steps.len() > 100 {
        return Err(AppError::Validation(
            "workflow definition cannot exceed 100 steps".to_string(),
        ));
    }

    let mut normalized_steps = Vec::with_capacity(parsed.steps.len());
    let mut seen_ids = std::collections::HashSet::new();

    for (index, step) in parsed.steps.into_iter().enumerate() {
        let step_id = required_trimmed("step.id", step.id, MAX_STEP_ID_LEN)?;
        if !is_valid_step_id(&step_id) {
            return Err(AppError::Validation(format!(
                "step[{index}] id contains invalid characters"
            )));
        }
        if !seen_ids.insert(step_id.clone()) {
            return Err(AppError::Validation(format!(
                "step[{index}] id '{step_id}' is duplicated"
            )));
        }

        let step_name = required_trimmed("step.name", step.name, MAX_STEP_NAME_LEN)?;
        let kind = parse_step_kind(step.kind)?;
        let approver_group = trim_optional(step.approver_group, 128);

        let auto_run_default =
            !matches!(kind, WorkflowStepKind::Manual | WorkflowStepKind::Approval);
        let auto_run = step.auto_run.unwrap_or(auto_run_default);

        let script = match kind {
            WorkflowStepKind::Script => {
                let script_value = required_trimmed(
                    "step.script",
                    step.script.unwrap_or_default(),
                    MAX_STEP_SCRIPT_LEN,
                )?;
                Some(script_value)
            }
            _ => None,
        };

        let timeout_seconds = step
            .timeout_seconds
            .unwrap_or(DEFAULT_SCRIPT_TIMEOUT_SECONDS)
            .clamp(1, MAX_SCRIPT_TIMEOUT_SECONDS);

        normalized_steps.push(WorkflowStep {
            id: step_id,
            name: step_name,
            kind,
            auto_run,
            script,
            timeout_seconds,
            approver_group,
        });
    }

    Ok(WorkflowDefinition {
        steps: normalized_steps,
    })
}

fn parse_workflow_definition(value: Value) -> AppResult<WorkflowDefinition> {
    let definition: WorkflowDefinition = serde_json::from_value(value).map_err(|err| {
        AppError::Validation(format!("stored workflow definition is invalid: {err}"))
    })?;

    if definition.steps.is_empty() {
        return Err(AppError::Validation(
            "stored workflow definition has no steps".to_string(),
        ));
    }

    Ok(definition)
}

fn parse_step_kind(value: String) -> AppResult<WorkflowStepKind> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "approval" => Ok(WorkflowStepKind::Approval),
        "script" => Ok(WorkflowStepKind::Script),
        "manual" => Ok(WorkflowStepKind::Manual),
        _ => Err(AppError::Validation(
            "step.kind must be one of: approval, script, manual".to_string(),
        )),
    }
}

fn normalize_request_payload(payload: Option<Value>) -> AppResult<Value> {
    let payload = payload.unwrap_or_else(|| json!({}));
    if !payload.is_object() {
        return Err(AppError::Validation(
            "payload must be a JSON object".to_string(),
        ));
    }
    Ok(payload)
}

fn workflow_step_kind_label(kind: WorkflowStepKind) -> &'static str {
    match kind {
        WorkflowStepKind::Approval => "approval",
        WorkflowStepKind::Script => "script",
        WorkflowStepKind::Manual => "manual",
    }
}

fn map_template_unique_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("workflow template name already exists".to_string());
        }
    }
    AppError::Database(err)
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
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            let clamped = clamp_chars(trimmed, max_len);
            Some(clamped)
        }
    })
}

fn is_valid_step_id(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn merge_stdout_stderr(stdout: &[u8], stderr: &[u8]) -> String {
    let out = String::from_utf8_lossy(stdout);
    let err = String::from_utf8_lossy(stderr);

    if out.is_empty() {
        return err.to_string();
    }
    if err.is_empty() {
        return out.to_string();
    }

    format!("{out}\n{err}")
}

fn truncate_text(mut text: String, max_len: usize) -> String {
    if text.len() <= max_len {
        return text;
    }

    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("...<truncated>");
    text
}

fn elapsed_to_i32(value: u128) -> i32 {
    value.min(i32::MAX as u128) as i32
}

fn clamp_chars(value: &str, max_len: usize) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_len {
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        ScriptExecutionPolicyMode, command_basename, hash_text_sha256, match_allowlist_command,
        parse_script_command, script_execution_policy_from_settings,
    };
    use crate::state::WorkflowExecutionSettings;

    #[test]
    fn parses_policy_mode() {
        assert_eq!(
            ScriptExecutionPolicyMode::from_str("disabled"),
            Some(ScriptExecutionPolicyMode::Disabled)
        );
        assert_eq!(
            ScriptExecutionPolicyMode::from_str("allowlist"),
            Some(ScriptExecutionPolicyMode::Allowlist)
        );
        assert_eq!(
            ScriptExecutionPolicyMode::from_str("sandboxed"),
            Some(ScriptExecutionPolicyMode::Sandboxed)
        );
        assert_eq!(ScriptExecutionPolicyMode::from_str("unknown"), None);
    }

    #[test]
    fn parses_script_command_from_shell_words() {
        let parsed = parse_script_command("echo \"hello world\" 123").expect("parse");
        assert_eq!(parsed.executable, "echo");
        assert_eq!(
            parsed.args,
            vec!["hello world".to_string(), "123".to_string()]
        );
    }

    #[test]
    fn parses_script_command_from_json_array() {
        let parsed = parse_script_command("[\"/bin/echo\", \"ok\"]").expect("parse");
        assert_eq!(parsed.executable, "/bin/echo");
        assert_eq!(parsed.args, vec!["ok".to_string()]);
    }

    #[test]
    fn matches_allowlist_by_full_path_or_basename() {
        let allowlist = vec!["/usr/bin/echo".to_string(), "printf".to_string()];
        assert_eq!(
            match_allowlist_command("/usr/bin/echo", &allowlist),
            Some("/usr/bin/echo".to_string())
        );
        assert_eq!(
            match_allowlist_command("echo", &allowlist),
            Some("/usr/bin/echo".to_string())
        );
        assert_eq!(
            match_allowlist_command("/bin/printf", &allowlist),
            Some("printf".to_string())
        );
        assert_eq!(match_allowlist_command("bash", &allowlist), None);
    }

    #[test]
    fn computes_stable_hash_length() {
        let digest = hash_text_sha256("echo hello");
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, hash_text_sha256("echo hello"));
    }

    #[test]
    fn normalizes_policy_from_settings() {
        let settings = WorkflowExecutionSettings {
            policy_mode: "allowlist".to_string(),
            allowlist: vec!["echo".to_string(), " ".to_string(), "".to_string()],
            sandbox_dir: "/tmp/cloudops-workflow-sandbox".to_string(),
        };

        let policy = script_execution_policy_from_settings(&settings);
        assert_eq!(policy.mode, ScriptExecutionPolicyMode::Allowlist);
        assert_eq!(policy.allowlist, vec!["echo".to_string()]);
        assert_eq!(policy.sandbox_dir, "/tmp/cloudops-workflow-sandbox");
    }

    #[test]
    fn resolves_command_basename() {
        assert_eq!(command_basename("/usr/bin/echo"), "echo");
        assert_eq!(command_basename("echo"), "echo");
    }
}
