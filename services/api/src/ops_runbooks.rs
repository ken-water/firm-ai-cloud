use std::collections::BTreeSet;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_TEMPLATE_KEY_LEN: usize = 64;
const MAX_TEXT_FIELD_LEN: usize = 256;
const MAX_NOTE_LEN: usize = 1024;
const MAX_TICKET_REF_LEN: usize = 128;
const MAX_ARTIFACT_URL_LEN: usize = 1024;
const DEFAULT_EXECUTION_LIMIT: u32 = 30;
const MAX_EXECUTION_LIMIT: u32 = 120;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/runbook-templates", get(list_runbook_templates))
        .route(
            "/cockpit/runbook-templates/executions",
            get(list_runbook_template_executions),
        )
        .route(
            "/cockpit/runbook-templates/executions/{id}",
            get(get_runbook_template_execution),
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
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Vec<RunbookStepTimelineEvent>,
    evidence: RunbookEvidenceRecord,
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
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Value,
    evidence: Value,
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

async fn list_runbook_templates() -> AppResult<Json<ListRunbookTemplatesResponse>> {
    let mut items = built_in_runbook_templates()
        .into_iter()
        .map(|template| runbook_template_to_catalog_item(&template))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.name.cmp(&right.name).then_with(|| left.key.cmp(&right.key)));

    Ok(Json(ListRunbookTemplatesResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
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
    let normalized_params = normalize_runbook_params(&template, payload.params)?;
    let preflight_confirmations =
        normalize_preflight_confirmations(&template, payload.preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);
    let evidence_input = normalize_runbook_evidence_input(payload.evidence)?;

    let now = Utc::now();
    let mut timeline = Vec::new();
    let mut remediation_hints = Vec::new();
    let mut final_status = "succeeded".to_string();

    for (idx, step) in template.steps.iter().enumerate() {
        let started_at = now + Duration::seconds(idx as i64);
        let finished_at = started_at + Duration::seconds(1);
        let failure_reason = evaluate_step_failure(
            template.key,
            step.step_id,
            &normalized_params,
            idx,
        );

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
            actor,
            params,
            preflight,
            timeline,
            evidence,
            remediation_hints,
            note
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id, template_key, template_name, status, actor, params, preflight,
                   timeline, evidence, remediation_hints, note, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(final_status.as_str())
    .bind(actor.as_str())
    .bind(Value::Object(normalized_params.clone()))
    .bind(preflight_snapshot)
    .bind(serde_json::to_value(&timeline).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook timeline: {err}"))
    })?)
    .bind(serde_json::to_value(&evidence).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook evidence: {err}"))
    })?)
    .bind(serde_json::to_value(&remediation_hints).map_err(|err| {
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
                "step_count": timeline.len(),
                "remediation_hint_count": remediation_hints.len(),
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

async fn list_runbook_template_executions(
    State(state): State<AppState>,
    Query(query): Query<ListRunbookTemplateExecutionsQuery>,
) -> AppResult<Json<ListRunbookTemplateExecutionsResponse>> {
    let template_key = query
        .template_key
        .map(normalize_template_key)
        .transpose()?;
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
        "SELECT id, template_key, template_name, status, actor, params, preflight,
                timeline, evidence, remediation_hints, note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE 1=1",
    );
    append_execution_filters(&mut list_builder, template_key, status);
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RunbookTemplateExecutionRow> = list_builder.build_query_as().fetch_all(&state.db).await?;
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
        "SELECT id, template_key, template_name, status, actor, params, preflight,
                timeline, evidence, remediation_hints, note, created_at, updated_at
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

fn parse_execution_row(row: RunbookTemplateExecutionRow) -> AppResult<RunbookTemplateExecutionItem> {
    let timeline: Vec<RunbookStepTimelineEvent> = serde_json::from_value(row.timeline).map_err(|err| {
        AppError::Validation(format!("runbook execution timeline data is invalid: {err}"))
    })?;
    let evidence: RunbookEvidenceRecord = serde_json::from_value(row.evidence).map_err(|err| {
        AppError::Validation(format!("runbook execution evidence data is invalid: {err}"))
    })?;
    let remediation_hints: Vec<String> = serde_json::from_value(row.remediation_hints).map_err(|err| {
        AppError::Validation(format!("runbook execution remediation_hints data is invalid: {err}"))
    })?;

    Ok(RunbookTemplateExecutionItem {
        id: row.id,
        template_key: row.template_key,
        template_name: row.template_name,
        status: row.status,
        actor: row.actor,
        params: row.params,
        preflight: row.preflight,
        timeline,
        evidence,
        remediation_hints,
        note: row.note,
        created_at: row.created_at,
        updated_at: row.updated_at,
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
        params: template
            .params
            .iter()
            .map(|param| RunbookTemplateParamItem {
                key: param.key.to_string(),
                label: param.label.to_string(),
                field_type: param.field_type.to_string(),
                required: param.required,
                options: param.options.iter().map(|item| (*item).to_string()).collect(),
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
        None => definition.default_value.map(|default| Value::String(default.to_string())),
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
                Value::Number(value) => value
                    .as_i64()
                    .ok_or_else(|| {
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
        evaluate_step_failure, normalize_preflight_confirmations, normalize_runbook_params,
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
        assert_eq!(normalized.get("probe_timeout_seconds").and_then(|value| value.as_i64()), Some(20));

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
        let err = normalize_preflight_confirmations(
            &template,
            vec!["confirm_change_window".to_string()],
        )
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

        let failure = evaluate_step_failure(
            template.key,
            "stakeholder_signoff",
            &params,
            1,
        );
        assert!(failure.is_some());
    }
}
