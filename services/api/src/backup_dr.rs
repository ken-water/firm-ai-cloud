use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_NOTE_LEN: usize = 1024;
const MAX_DESTINATION_URI_LEN: usize = 512;
const MAX_POLICY_KEY_LEN: usize = 64;
const MAX_POLICY_NAME_LEN: usize = 128;
const MAX_POLICY_MODE_LEN: usize = 32;
const MAX_TICKET_REF_LEN: usize = 128;
const MAX_ARTIFACT_URL_LEN: usize = 1024;
const MAX_VERIFIER_LEN: usize = 128;
const DEFAULT_RUN_LIMIT: u32 = 40;
const MAX_RUN_LIMIT: u32 = 200;
const DEFAULT_EVIDENCE_LIMIT: u32 = 60;
const MAX_EVIDENCE_LIMIT: u32 = 200;
const MAX_EVIDENCE_COMPLIANCE_SLA_HOURS: i32 = 720;
const DEFAULT_EVIDENCE_COMPLIANCE_SLA_HOURS: i32 = 24;
const EVIDENCE_COMPLIANCE_POLICY_KEY: &str = "global";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/cockpit/backup/policies",
            get(list_backup_policies).post(create_backup_policy),
        )
        .route("/cockpit/backup/policies/{id}", patch(update_backup_policy))
        .route("/cockpit/backup/policies/{id}/run", post(run_backup_policy))
        .route("/cockpit/backup/runs", get(list_backup_policy_runs))
        .route(
            "/cockpit/backup/runs/{id}/restore-evidence",
            post(create_restore_evidence),
        )
        .route("/cockpit/backup/restore-evidence", get(list_restore_evidence))
        .route(
            "/cockpit/backup/restore-evidence/{id}",
            patch(update_restore_evidence),
        )
        .route(
            "/cockpit/backup/evidence-compliance/policy",
            get(get_restore_evidence_compliance_policy)
                .put(update_restore_evidence_compliance_policy),
        )
        .route(
            "/cockpit/backup/evidence-compliance/scorecard",
            get(get_restore_evidence_compliance_scorecard),
        )
        .route(
            "/cockpit/backup/evidence-compliance/scorecard/export",
            get(export_restore_evidence_compliance_scorecard),
        )
        .route("/cockpit/backup/scheduler/tick", post(run_backup_scheduler_tick))
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct BackupPolicyRecord {
    id: i64,
    policy_key: String,
    name: String,
    frequency: String,
    schedule_time_utc: String,
    schedule_weekday: Option<i16>,
    retention_days: i32,
    destination_type: String,
    destination_uri: String,
    destination_validated: bool,
    drill_enabled: bool,
    drill_frequency: String,
    drill_weekday: Option<i16>,
    drill_time_utc: String,
    last_backup_status: String,
    last_backup_at: Option<DateTime<Utc>>,
    last_backup_error: Option<String>,
    last_drill_status: String,
    last_drill_at: Option<DateTime<Utc>>,
    last_drill_error: Option<String>,
    next_backup_at: Option<DateTime<Utc>>,
    next_drill_at: Option<DateTime<Utc>>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct BackupPolicyRunRecord {
    id: i64,
    policy_id: i64,
    run_type: String,
    status: String,
    triggered_by: String,
    triggered_by_scheduler: bool,
    note: Option<String>,
    remediation_hint: Option<String>,
    error_message: Option<String>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    restore_evidence_count: i64,
    latest_restore_verified_at: Option<DateTime<Utc>>,
    latest_restore_closure_status: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct BackupRestoreEvidenceRecord {
    id: i64,
    run_id: i64,
    policy_id: i64,
    run_type: String,
    run_status: String,
    ticket_ref: Option<String>,
    artifact_url: String,
    note: Option<String>,
    verifier: String,
    closure_status: String,
    closed_at: Option<DateTime<Utc>>,
    closed_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListBackupPoliciesResponse {
    generated_at: DateTime<Utc>,
    total: usize,
    items: Vec<BackupPolicyRecord>,
}

#[derive(Debug, Serialize)]
struct ListBackupPolicyRunsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<BackupPolicyRunRecord>,
}

#[derive(Debug, Serialize)]
struct RestoreEvidenceCoverageSummary {
    required_runs: i64,
    covered_runs: i64,
    missing_runs: i64,
}

#[derive(Debug, Serialize)]
struct ListRestoreEvidenceResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    coverage: RestoreEvidenceCoverageSummary,
    missing_run_ids: Vec<i64>,
    items: Vec<BackupRestoreEvidenceRecord>,
}

#[derive(Debug, Deserialize)]
struct CreateBackupPolicyRequest {
    policy_key: String,
    name: String,
    frequency: String,
    schedule_time_utc: String,
    schedule_weekday: Option<i16>,
    retention_days: i32,
    destination_type: String,
    destination_uri: String,
    drill_enabled: Option<bool>,
    drill_frequency: Option<String>,
    drill_weekday: Option<i16>,
    drill_time_utc: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateBackupPolicyRequest {
    name: Option<String>,
    frequency: Option<String>,
    schedule_time_utc: Option<String>,
    schedule_weekday: Option<i16>,
    retention_days: Option<i32>,
    destination_type: Option<String>,
    destination_uri: Option<String>,
    drill_enabled: Option<bool>,
    drill_frequency: Option<String>,
    drill_weekday: Option<i16>,
    drill_time_utc: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunBackupPolicyRequest {
    run_type: String,
    simulate_failure: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListBackupPolicyRunsQuery {
    policy_id: Option<i64>,
    run_type: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ListRestoreEvidenceQuery {
    policy_id: Option<i64>,
    run_status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RestoreEvidenceComplianceScorecardQuery {
    week_start: Option<String>,
    as_of: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ExportRestoreEvidenceComplianceScorecardQuery {
    week_start: Option<String>,
    as_of: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct BackupSchedulerTickRequest {
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateRestoreEvidenceRequest {
    ticket_ref: Option<String>,
    artifact_url: String,
    note: Option<String>,
    verifier: Option<String>,
    close_evidence: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateRestoreEvidenceRequest {
    ticket_ref: Option<String>,
    artifact_url: Option<String>,
    note: Option<String>,
    verifier: Option<String>,
    close_evidence: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct UpdateRestoreEvidenceCompliancePolicyRequest {
    mode: Option<String>,
    sla_hours: Option<i32>,
    require_failed_runs: Option<bool>,
    require_drill_runs: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunBackupPolicyResponse {
    policy: BackupPolicyRecord,
    run: BackupPolicyRunRecord,
    remediation_hints: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BackupSchedulerTickResponse {
    generated_at: DateTime<Utc>,
    backup_runs: usize,
    drill_runs: usize,
    runs: Vec<BackupPolicyRunRecord>,
}

#[derive(Debug, Serialize, Clone)]
struct RestoreEvidenceCompliancePolicyView {
    policy_key: String,
    mode: String,
    sla_hours: i32,
    require_failed_runs: bool,
    require_drill_runs: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RestoreEvidenceCompliancePolicyResponse {
    generated_at: DateTime<Utc>,
    policy: RestoreEvidenceCompliancePolicyView,
}

#[derive(Debug, Serialize, Clone)]
struct RestoreEvidenceComplianceMetrics {
    required_runs: i64,
    closed_runs: i64,
    closed_within_sla_runs: i64,
    open_runs: i64,
    overdue_runs: i64,
    overdue_open_runs: i64,
}

#[derive(Debug, Serialize, Clone)]
struct RestoreEvidenceComplianceScorecardTimelinePoint {
    date: String,
    required_runs: i64,
    closed_runs: i64,
    overdue_runs: i64,
}

#[derive(Debug, Serialize, Clone)]
struct RestoreEvidenceComplianceItem {
    run_id: i64,
    policy_id: i64,
    run_type: String,
    run_status: String,
    started_at: DateTime<Utc>,
    deadline_at: DateTime<Utc>,
    evidence_total: i64,
    closed_evidence_count: i64,
    closure_state: String,
    closed_at: Option<DateTime<Utc>>,
    latest_evidence_id: Option<i64>,
    latest_evidence_at: Option<DateTime<Utc>>,
    latest_closure_status: Option<String>,
    overdue_hours: i64,
    run_ref: String,
}

#[derive(Debug, Serialize)]
struct RestoreEvidenceComplianceScorecardResponse {
    generated_at: DateTime<Utc>,
    scorecard_key: String,
    week_start: String,
    week_end: String,
    as_of: DateTime<Utc>,
    policy: RestoreEvidenceCompliancePolicyView,
    metrics: RestoreEvidenceComplianceMetrics,
    timeline: Vec<RestoreEvidenceComplianceScorecardTimelinePoint>,
    overdue_items: Vec<RestoreEvidenceComplianceItem>,
}

#[derive(Debug, Serialize)]
struct RestoreEvidenceComplianceScorecardExportResponse {
    generated_at: DateTime<Utc>,
    scorecard_key: String,
    format: String,
    content: String,
}

#[derive(Debug, FromRow)]
struct RestoreEvidenceCompliancePolicyRow {
    policy_key: String,
    mode: String,
    sla_hours: i32,
    require_failed_runs: bool,
    require_drill_runs: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct RestoreEvidenceCompliancePolicy {
    policy_key: String,
    mode: String,
    sla_hours: i32,
    require_failed_runs: bool,
    require_drill_runs: bool,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RestoreEvidenceComplianceRunRow {
    run_id: i64,
    policy_id: i64,
    run_type: String,
    run_status: String,
    started_at: DateTime<Utc>,
    evidence_total: i64,
    closed_evidence_count: i64,
    first_closed_at: Option<DateTime<Utc>>,
    latest_evidence_id: Option<i64>,
    latest_closure_status: Option<String>,
    latest_evidence_at: Option<DateTime<Utc>>,
}

async fn list_backup_policies(
    State(state): State<AppState>,
) -> AppResult<Json<ListBackupPoliciesResponse>> {
    let items: Vec<BackupPolicyRecord> = sqlx::query_as(
        "SELECT id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                retention_days, destination_type, destination_uri, destination_validated,
                drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                last_backup_status, last_backup_at, last_backup_error,
                last_drill_status, last_drill_at, last_drill_error,
                next_backup_at, next_drill_at, updated_by, created_at, updated_at
         FROM ops_backup_policies
         ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ListBackupPoliciesResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
    }))
}

async fn create_backup_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateBackupPolicyRequest>,
) -> AppResult<Json<BackupPolicyRecord>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let policy_key = normalize_policy_key(payload.policy_key)?;
    let name = required_trimmed("name", payload.name, MAX_POLICY_NAME_LEN)?;
    let frequency = normalize_backup_frequency(payload.frequency)?;
    let schedule_time_utc = normalize_hhmm(payload.schedule_time_utc, "schedule_time_utc")?;
    let schedule_weekday = normalize_schedule_weekday(payload.schedule_weekday, frequency.as_str())?;
    let retention_days = normalize_retention_days(payload.retention_days)?;
    let destination_type = normalize_destination_type(payload.destination_type)?;
    let destination_uri = normalize_destination_uri(payload.destination_uri)?;
    validate_destination_uri(destination_type.as_str(), destination_uri.as_str())?;

    let drill_enabled = payload.drill_enabled.unwrap_or(true);
    let drill_frequency = normalize_drill_frequency(payload.drill_frequency)?;
    let drill_time_utc = normalize_hhmm(
        payload.drill_time_utc.unwrap_or_else(|| "02:00".to_string()),
        "drill_time_utc",
    )?;
    let drill_weekday = normalize_drill_weekday(payload.drill_weekday, drill_frequency.as_str())?;
    let note = normalize_optional_note(payload.note)?;

    let now = Utc::now();
    let next_backup_at = compute_next_backup_at(
        frequency.as_str(),
        schedule_time_utc.as_str(),
        schedule_weekday,
        now,
    )?;
    let next_drill_at = if drill_enabled {
        Some(compute_next_drill_at(
            drill_frequency.as_str(),
            drill_time_utc.as_str(),
            drill_weekday,
            now,
        )?)
    } else {
        None
    };

    let item: BackupPolicyRecord = sqlx::query_as(
        "INSERT INTO ops_backup_policies (
            policy_key, name, frequency, schedule_time_utc, schedule_weekday,
            retention_days, destination_type, destination_uri, destination_validated,
            drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
            next_backup_at, next_drill_at, updated_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9, $10, $11, $12, $13, $14, $15)
         RETURNING id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                   retention_days, destination_type, destination_uri, destination_validated,
                   drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                   last_backup_status, last_backup_at, last_backup_error,
                   last_drill_status, last_drill_at, last_drill_error,
                   next_backup_at, next_drill_at, updated_by, created_at, updated_at",
    )
    .bind(policy_key.clone())
    .bind(name)
    .bind(frequency)
    .bind(schedule_time_utc)
    .bind(schedule_weekday)
    .bind(retention_days)
    .bind(destination_type)
    .bind(destination_uri)
    .bind(drill_enabled)
    .bind(drill_frequency)
    .bind(drill_weekday)
    .bind(drill_time_utc)
    .bind(next_backup_at)
    .bind(next_drill_at)
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.backup.policy.create".to_string(),
            target_type: "ops_backup_policy".to_string(),
            target_id: Some(item.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "policy_key": policy_key,
                "frequency": item.frequency,
                "retention_days": item.retention_days,
                "destination_type": item.destination_type,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn update_backup_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateBackupPolicyRequest>,
) -> AppResult<Json<BackupPolicyRecord>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = load_backup_policy(&state.db, id).await?;
    let frequency_changed = payload.frequency.is_some();
    let drill_frequency_changed = payload.drill_frequency.is_some();

    let name = payload
        .name
        .map(|value| required_trimmed("name", value, MAX_POLICY_NAME_LEN))
        .transpose()?
        .unwrap_or(current.name.clone());

    let frequency = payload
        .frequency
        .clone()
        .map(normalize_backup_frequency)
        .transpose()?
        .unwrap_or(current.frequency.clone());

    let schedule_time_utc = payload
        .schedule_time_utc
        .map(|value| normalize_hhmm(value, "schedule_time_utc"))
        .transpose()?
        .unwrap_or(current.schedule_time_utc.clone());

    let schedule_weekday = if payload.schedule_weekday.is_some() || frequency_changed {
        normalize_schedule_weekday(payload.schedule_weekday, frequency.as_str())?
    } else {
        current.schedule_weekday
    };

    let retention_days = payload
        .retention_days
        .map(normalize_retention_days)
        .transpose()?
        .unwrap_or(current.retention_days);

    let destination_type = payload
        .destination_type
        .map(normalize_destination_type)
        .transpose()?
        .unwrap_or(current.destination_type.clone());

    let destination_uri = payload
        .destination_uri
        .map(normalize_destination_uri)
        .transpose()?
        .unwrap_or(current.destination_uri.clone());
    validate_destination_uri(destination_type.as_str(), destination_uri.as_str())?;

    let drill_enabled = payload.drill_enabled.unwrap_or(current.drill_enabled);

    let drill_frequency = payload
        .drill_frequency
        .clone()
        .map(|value| normalize_drill_frequency(Some(value)))
        .transpose()?
        .unwrap_or(current.drill_frequency.clone());

    let drill_time_utc = payload
        .drill_time_utc
        .map(|value| normalize_hhmm(value, "drill_time_utc"))
        .transpose()?
        .unwrap_or(current.drill_time_utc.clone());

    let drill_weekday = if payload.drill_weekday.is_some() || drill_frequency_changed {
        normalize_drill_weekday(payload.drill_weekday, drill_frequency.as_str())?
    } else {
        current.drill_weekday
    };

    let note = normalize_optional_note(payload.note)?;

    let now = Utc::now();
    let next_backup_at = compute_next_backup_at(
        frequency.as_str(),
        schedule_time_utc.as_str(),
        schedule_weekday,
        now,
    )?;
    let next_drill_at = if drill_enabled {
        Some(compute_next_drill_at(
            drill_frequency.as_str(),
            drill_time_utc.as_str(),
            drill_weekday,
            now,
        )?)
    } else {
        None
    };

    let item: BackupPolicyRecord = sqlx::query_as(
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
             next_backup_at = $13,
             next_drill_at = $14,
             updated_by = $15,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                   retention_days, destination_type, destination_uri, destination_validated,
                   drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                   last_backup_status, last_backup_at, last_backup_error,
                   last_drill_status, last_drill_at, last_drill_error,
                   next_backup_at, next_drill_at, updated_by, created_at, updated_at",
    )
    .bind(id)
    .bind(name)
    .bind(frequency)
    .bind(schedule_time_utc)
    .bind(schedule_weekday)
    .bind(retention_days)
    .bind(destination_type)
    .bind(destination_uri)
    .bind(drill_enabled)
    .bind(drill_frequency)
    .bind(drill_weekday)
    .bind(drill_time_utc)
    .bind(next_backup_at)
    .bind(next_drill_at)
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.backup.policy.update".to_string(),
            target_type: "ops_backup_policy".to_string(),
            target_id: Some(item.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "policy_key": item.policy_key,
                "frequency": item.frequency,
                "retention_days": item.retention_days,
                "destination_type": item.destination_type,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn run_backup_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<RunBackupPolicyRequest>,
) -> AppResult<Json<RunBackupPolicyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let run_type = normalize_run_type(payload.run_type)?;
    let simulate_failure = payload.simulate_failure.unwrap_or(false);
    let note = normalize_optional_note(payload.note)?;

    let (policy, run, remediation_hints) =
        execute_backup_run(&state.db, id, run_type.as_str(), actor.as_str(), false, simulate_failure, note.clone())
            .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.backup.run.manual".to_string(),
            target_type: "ops_backup_policy".to_string(),
            target_id: Some(id.to_string()),
            result: run.status.clone(),
            message: note,
            metadata: json!({
                "run_id": run.id,
                "run_type": run.run_type,
                "status": run.status,
                "policy_key": policy.policy_key,
            }),
        },
    )
    .await;

    Ok(Json(RunBackupPolicyResponse {
        policy,
        run,
        remediation_hints,
    }))
}

async fn run_backup_scheduler_tick(
    State(state): State<AppState>,
    Json(payload): Json<BackupSchedulerTickRequest>,
) -> AppResult<Json<BackupSchedulerTickResponse>> {
    let note = normalize_optional_note(payload.note)?;
    let now = Utc::now();

    let due_backup_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_backup_policies
         WHERE next_backup_at IS NOT NULL
           AND next_backup_at <= $1
         ORDER BY next_backup_at ASC, id ASC",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;

    let due_drill_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_backup_policies
         WHERE drill_enabled = TRUE
           AND next_drill_at IS NOT NULL
           AND next_drill_at <= $1
         ORDER BY next_drill_at ASC, id ASC",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;

    let mut runs = Vec::new();
    for policy_id in due_backup_ids.iter().copied() {
        let (_, run, _) = execute_backup_run(
            &state.db,
            policy_id,
            "backup",
            "system:scheduler",
            true,
            false,
            note.clone(),
        )
        .await?;
        runs.push(run);
    }
    let backup_runs = runs.len();

    for policy_id in due_drill_ids.iter().copied() {
        let (_, run, _) = execute_backup_run(
            &state.db,
            policy_id,
            "drill",
            "system:scheduler",
            true,
            false,
            note.clone(),
        )
        .await?;
        runs.push(run);
    }
    let drill_runs = runs.len().saturating_sub(backup_runs);

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: "system:scheduler".to_string(),
            action: "ops.backup.scheduler.tick".to_string(),
            target_type: "ops_backup_policy".to_string(),
            target_id: None,
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "backup_runs": backup_runs,
                "drill_runs": drill_runs,
            }),
        },
    )
    .await;

    Ok(Json(BackupSchedulerTickResponse {
        generated_at: Utc::now(),
        backup_runs,
        drill_runs,
        runs,
    }))
}

async fn list_backup_policy_runs(
    State(state): State<AppState>,
    Query(query): Query<ListBackupPolicyRunsQuery>,
) -> AppResult<Json<ListBackupPolicyRunsResponse>> {
    let run_type = query
        .run_type
        .map(normalize_run_type)
        .transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_RUN_LIMIT)
        .clamp(1, MAX_RUN_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_backup_policy_runs r WHERE 1=1");
    append_backup_run_filters(&mut count_builder, query.policy_id, run_type.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, policy_id, run_type, status, triggered_by, triggered_by_scheduler,
                note, remediation_hint, error_message, started_at, finished_at,
                (SELECT COUNT(*) FROM ops_backup_restore_evidence e WHERE e.run_id = r.id) AS restore_evidence_count,
                (SELECT e.created_at
                 FROM ops_backup_restore_evidence e
                 WHERE e.run_id = r.id
                 ORDER BY e.created_at DESC, e.id DESC
                 LIMIT 1) AS latest_restore_verified_at,
                (SELECT e.closure_status
                 FROM ops_backup_restore_evidence e
                 WHERE e.run_id = r.id
                 ORDER BY e.created_at DESC, e.id DESC
                 LIMIT 1) AS latest_restore_closure_status,
                created_at
         FROM ops_backup_policy_runs r
         WHERE 1=1",
    );
    append_backup_run_filters(&mut list_builder, query.policy_id, run_type);
    list_builder
        .push(" ORDER BY r.started_at DESC, r.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<BackupPolicyRunRecord> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListBackupPolicyRunsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn create_restore_evidence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<CreateRestoreEvidenceRequest>,
) -> AppResult<Json<BackupRestoreEvidenceRecord>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let run = load_backup_run(&state.db, id).await?;
    let ticket_ref = trim_optional(payload.ticket_ref, MAX_TICKET_REF_LEN);
    let artifact_url = required_trimmed("artifact_url", payload.artifact_url, MAX_ARTIFACT_URL_LEN)?;
    let note = normalize_optional_note(payload.note)?;
    let verifier = trim_optional(payload.verifier, MAX_VERIFIER_LEN)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| actor.clone());
    let close_evidence = payload.close_evidence.unwrap_or(false);
    let evidence_policy = load_or_init_restore_evidence_compliance_policy(&state.db).await?;
    enforce_restore_evidence_compliance(&evidence_policy, &run, close_evidence, Utc::now())?;
    let closure_status = if close_evidence { "closed" } else { "open" };

    let item: BackupRestoreEvidenceRecord = sqlx::query_as(
        "INSERT INTO ops_backup_restore_evidence (
            run_id, policy_id, run_type, run_status, ticket_ref, artifact_url,
            note, verifier, closure_status, closed_at, closed_by, metadata
         )
         VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9,
            CASE WHEN $9 = 'closed' THEN NOW() ELSE NULL END,
            CASE WHEN $9 = 'closed' THEN $10 ELSE NULL END,
            $11
         )
         RETURNING id, run_id, policy_id, run_type, run_status, ticket_ref, artifact_url,
                   note, verifier, closure_status, closed_at, closed_by, created_at, updated_at",
    )
    .bind(run.id)
    .bind(run.policy_id)
    .bind(run.run_type.as_str())
    .bind(run.status.as_str())
    .bind(ticket_ref.clone())
    .bind(artifact_url.clone())
    .bind(note.clone())
    .bind(verifier.clone())
    .bind(closure_status)
    .bind(actor.clone())
    .bind(json!({
        "policy_id": run.policy_id,
        "run_type": run.run_type,
        "run_status": run.status,
        "closed_on_create": close_evidence,
    }))
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.backup.restore_evidence.create".to_string(),
            target_type: "ops_backup_restore_evidence".to_string(),
            target_id: Some(item.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "run_id": run.id,
                "policy_id": run.policy_id,
                "run_type": run.run_type,
                "run_status": run.status,
                "ticket_ref": ticket_ref,
                "artifact_url": artifact_url,
                "verifier": verifier,
                "closure_status": item.closure_status,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn update_restore_evidence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateRestoreEvidenceRequest>,
) -> AppResult<Json<BackupRestoreEvidenceRecord>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = load_restore_evidence(&state.db, id).await?;
    if current.closure_status == "closed" {
        return Err(AppError::Validation(
            "restore evidence is immutable after closure".to_string(),
        ));
    }

    let ticket_ref = payload
        .ticket_ref
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.len() > MAX_TICKET_REF_LEN {
                Some(trimmed[..MAX_TICKET_REF_LEN].to_string())
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or(current.ticket_ref.clone());

    let artifact_url = payload
        .artifact_url
        .map(|value| required_trimmed("artifact_url", value, MAX_ARTIFACT_URL_LEN))
        .transpose()?
        .unwrap_or(current.artifact_url.clone());

    let note = match payload.note {
        Some(value) => normalize_optional_note(Some(value))?,
        None => current.note.clone(),
    };

    let verifier = payload
        .verifier
        .map(|value| required_trimmed("verifier", value, MAX_VERIFIER_LEN))
        .transpose()?
        .unwrap_or(current.verifier.clone());

    let close_evidence = payload.close_evidence.unwrap_or(false);
    let run = load_backup_run(&state.db, current.run_id).await?;
    let evidence_policy = load_or_init_restore_evidence_compliance_policy(&state.db).await?;
    enforce_restore_evidence_compliance(&evidence_policy, &run, close_evidence, Utc::now())?;
    let closure_status = if close_evidence { "closed" } else { "open" };

    let item: BackupRestoreEvidenceRecord = sqlx::query_as(
        "UPDATE ops_backup_restore_evidence
         SET ticket_ref = $2,
             artifact_url = $3,
             note = $4,
             verifier = $5,
             closure_status = $6,
             closed_at = CASE WHEN $6 = 'closed' THEN NOW() ELSE NULL END,
             closed_by = CASE WHEN $6 = 'closed' THEN $7 ELSE NULL END,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, run_id, policy_id, run_type, run_status, ticket_ref, artifact_url,
                   note, verifier, closure_status, closed_at, closed_by, created_at, updated_at",
    )
    .bind(id)
    .bind(ticket_ref.clone())
    .bind(artifact_url.clone())
    .bind(note.clone())
    .bind(verifier.clone())
    .bind(closure_status)
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.backup.restore_evidence.update".to_string(),
            target_type: "ops_backup_restore_evidence".to_string(),
            target_id: Some(item.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "run_id": item.run_id,
                "policy_id": item.policy_id,
                "ticket_ref": ticket_ref,
                "artifact_url": artifact_url,
                "verifier": verifier,
                "closure_status": item.closure_status,
                "updated_by": actor,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn list_restore_evidence(
    State(state): State<AppState>,
    Query(query): Query<ListRestoreEvidenceQuery>,
) -> AppResult<Json<ListRestoreEvidenceResponse>> {
    let run_status = query
        .run_status
        .map(normalize_run_status)
        .transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EVIDENCE_LIMIT)
        .clamp(1, MAX_EVIDENCE_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_backup_restore_evidence e WHERE 1=1");
    append_restore_evidence_filters(&mut count_builder, query.policy_id, run_status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, run_id, policy_id, run_type, run_status, ticket_ref, artifact_url, note,
                verifier, closure_status, closed_at, closed_by, created_at, updated_at
         FROM ops_backup_restore_evidence e
         WHERE 1=1",
    );
    append_restore_evidence_filters(&mut list_builder, query.policy_id, run_status.clone());
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<BackupRestoreEvidenceRecord> = list_builder.build_query_as().fetch_all(&state.db).await?;

    let required_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policy_runs r
         WHERE r.status = 'failed'
            OR r.run_type = 'drill'",
    )
    .fetch_one(&state.db)
    .await?;

    let covered_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT e.run_id)
         FROM ops_backup_restore_evidence e
         INNER JOIN ops_backup_policy_runs r ON r.id = e.run_id
         WHERE r.status = 'failed'
            OR r.run_type = 'drill'",
    )
    .fetch_one(&state.db)
    .await?;

    let missing_run_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT r.id
         FROM ops_backup_policy_runs r
         WHERE (r.status = 'failed' OR r.run_type = 'drill')
           AND NOT EXISTS (
               SELECT 1
               FROM ops_backup_restore_evidence e
               WHERE e.run_id = r.id
           )
         ORDER BY r.started_at DESC, r.id DESC
         LIMIT 80",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ListRestoreEvidenceResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        coverage: RestoreEvidenceCoverageSummary {
            required_runs,
            covered_runs,
            missing_runs: (required_runs - covered_runs).max(0),
        },
        missing_run_ids,
        items,
    }))
}

async fn get_restore_evidence_compliance_policy(
    State(state): State<AppState>,
) -> AppResult<Json<RestoreEvidenceCompliancePolicyResponse>> {
    let policy = load_or_init_restore_evidence_compliance_policy(&state.db).await?;
    Ok(Json(RestoreEvidenceCompliancePolicyResponse {
        generated_at: policy.updated_at,
        policy: build_restore_evidence_policy_view(&policy),
    }))
}

async fn update_restore_evidence_compliance_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRestoreEvidenceCompliancePolicyRequest>,
) -> AppResult<Json<RestoreEvidenceCompliancePolicyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = load_or_init_restore_evidence_compliance_policy(&state.db).await?;
    let mode = payload
        .mode
        .map(normalize_restore_evidence_policy_mode)
        .transpose()?
        .unwrap_or_else(|| current.mode.clone());
    let sla_hours = payload
        .sla_hours
        .map(normalize_restore_evidence_sla_hours)
        .transpose()?
        .unwrap_or(current.sla_hours);
    let require_failed_runs = payload
        .require_failed_runs
        .unwrap_or(current.require_failed_runs);
    let require_drill_runs = payload
        .require_drill_runs
        .unwrap_or(current.require_drill_runs);
    if !require_failed_runs && !require_drill_runs {
        return Err(AppError::Validation(
            "at least one selector is required: require_failed_runs or require_drill_runs"
                .to_string(),
        ));
    }

    let note = normalize_optional_note(payload.note)?;
    let updated_row: RestoreEvidenceCompliancePolicyRow = sqlx::query_as(
        "UPDATE ops_restore_evidence_compliance_policies
         SET mode = $2,
             sla_hours = $3,
             require_failed_runs = $4,
             require_drill_runs = $5,
             updated_by = $6,
             updated_at = NOW()
         WHERE policy_key = $1
         RETURNING policy_key, mode, sla_hours, require_failed_runs, require_drill_runs, updated_by, updated_at",
    )
    .bind(EVIDENCE_COMPLIANCE_POLICY_KEY)
    .bind(mode.as_str())
    .bind(sla_hours)
    .bind(require_failed_runs)
    .bind(require_drill_runs)
    .bind(actor.as_str())
    .fetch_one(&state.db)
    .await?;
    let policy = parse_restore_evidence_compliance_policy_row(updated_row)?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.backup.restore_evidence.policy.update".to_string(),
            target_type: "ops_restore_evidence_compliance_policy".to_string(),
            target_id: Some(policy.policy_key.clone()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "mode": policy.mode,
                "sla_hours": policy.sla_hours,
                "require_failed_runs": policy.require_failed_runs,
                "require_drill_runs": policy.require_drill_runs,
                "updated_by": actor,
            }),
        },
    )
    .await;

    Ok(Json(RestoreEvidenceCompliancePolicyResponse {
        generated_at: policy.updated_at,
        policy: build_restore_evidence_policy_view(&policy),
    }))
}

async fn get_restore_evidence_compliance_scorecard(
    State(state): State<AppState>,
    Query(query): Query<RestoreEvidenceComplianceScorecardQuery>,
) -> AppResult<Json<RestoreEvidenceComplianceScorecardResponse>> {
    let week_start = parse_scorecard_week_start(query.week_start)?;
    let as_of = parse_optional_as_of(query.as_of)?.unwrap_or_else(Utc::now);
    let scorecard =
        build_restore_evidence_compliance_scorecard(&state.db, week_start, as_of).await?;
    Ok(Json(scorecard))
}

async fn export_restore_evidence_compliance_scorecard(
    State(state): State<AppState>,
    Query(query): Query<ExportRestoreEvidenceComplianceScorecardQuery>,
) -> AppResult<Json<RestoreEvidenceComplianceScorecardExportResponse>> {
    let week_start = parse_scorecard_week_start(query.week_start)?;
    let as_of = parse_optional_as_of(query.as_of)?.unwrap_or_else(Utc::now);
    let scorecard =
        build_restore_evidence_compliance_scorecard(&state.db, week_start, as_of).await?;
    let format = query
        .format
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_ascii_lowercase();
    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&scorecard).map_err(|err| {
            AppError::Validation(format!(
                "failed to serialize evidence compliance scorecard json: {err}"
            ))
        })?,
        "csv" => restore_evidence_scorecard_to_csv(&scorecard),
        _ => {
            return Err(AppError::Validation(
                "format must be one of: csv, json".to_string(),
            ));
        }
    };

    Ok(Json(RestoreEvidenceComplianceScorecardExportResponse {
        generated_at: scorecard.generated_at,
        scorecard_key: scorecard.scorecard_key,
        format,
        content,
    }))
}

async fn build_restore_evidence_compliance_scorecard(
    db: &sqlx::PgPool,
    week_start: NaiveDate,
    as_of: DateTime<Utc>,
) -> AppResult<RestoreEvidenceComplianceScorecardResponse> {
    let policy = load_or_init_restore_evidence_compliance_policy(db).await?;
    let week_end = week_start + Duration::days(6);
    let range_start = Utc.from_utc_datetime(
        &week_start
            .and_hms_opt(0, 0, 0)
            .expect("valid start of day"),
    );
    let range_end_exclusive = Utc.from_utc_datetime(
        &(week_end + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .expect("valid end exclusive"),
    );

    let rows: Vec<RestoreEvidenceComplianceRunRow> = sqlx::query_as(
        "SELECT r.id AS run_id,
                r.policy_id,
                r.run_type,
                r.status AS run_status,
                r.started_at,
                COALESCE(stats.evidence_total, 0)::bigint AS evidence_total,
                COALESCE(stats.closed_evidence_count, 0)::bigint AS closed_evidence_count,
                stats.first_closed_at,
                stats.latest_evidence_id,
                stats.latest_closure_status,
                stats.latest_evidence_at
         FROM ops_backup_policy_runs r
         LEFT JOIN LATERAL (
            SELECT COUNT(*)::bigint AS evidence_total,
                   COUNT(*) FILTER (WHERE e.closure_status = 'closed')::bigint AS closed_evidence_count,
                   MIN(e.closed_at) FILTER (WHERE e.closure_status = 'closed') AS first_closed_at,
                   (
                     SELECT e2.id
                     FROM ops_backup_restore_evidence e2
                     WHERE e2.run_id = r.id
                     ORDER BY e2.created_at DESC, e2.id DESC
                     LIMIT 1
                   ) AS latest_evidence_id,
                   (
                     SELECT e2.closure_status
                     FROM ops_backup_restore_evidence e2
                     WHERE e2.run_id = r.id
                     ORDER BY e2.created_at DESC, e2.id DESC
                     LIMIT 1
                   ) AS latest_closure_status,
                   (
                     SELECT e2.created_at
                     FROM ops_backup_restore_evidence e2
                     WHERE e2.run_id = r.id
                     ORDER BY e2.created_at DESC, e2.id DESC
                     LIMIT 1
                   ) AS latest_evidence_at
            FROM ops_backup_restore_evidence e
            WHERE e.run_id = r.id
         ) stats ON TRUE
         WHERE r.started_at >= $1
           AND r.started_at < $2
           AND (
                ($3 = TRUE AND r.status = 'failed')
                OR
                ($4 = TRUE AND r.run_type = 'drill')
           )
         ORDER BY r.started_at DESC, r.id DESC",
    )
    .bind(range_start)
    .bind(range_end_exclusive)
    .bind(policy.require_failed_runs)
    .bind(policy.require_drill_runs)
    .fetch_all(db)
    .await?;

    let mut items = Vec::new();
    for row in rows {
        let deadline_at = row.started_at + Duration::hours(policy.sla_hours as i64);
        let closed_at = row.first_closed_at;
        let (closure_state, overdue_hours) =
            evaluate_restore_evidence_closure_state(deadline_at, closed_at, as_of);
        items.push(RestoreEvidenceComplianceItem {
            run_id: row.run_id,
            policy_id: row.policy_id,
            run_type: row.run_type,
            run_status: row.run_status,
            started_at: row.started_at,
            deadline_at,
            evidence_total: row.evidence_total,
            closed_evidence_count: row.closed_evidence_count,
            closure_state,
            closed_at,
            latest_evidence_id: row.latest_evidence_id,
            latest_evidence_at: row.latest_evidence_at,
            latest_closure_status: row.latest_closure_status,
            overdue_hours,
            run_ref: format!("/api/v1/ops/cockpit/backup/runs/{}", row.run_id),
        });
    }

    let mut timeline = Vec::new();
    let mut day = week_start;
    while day <= week_end {
        let day_items = items
            .iter()
            .filter(|item| item.started_at.date_naive() == day)
            .collect::<Vec<_>>();
        let required_runs = day_items.len() as i64;
        let closed_runs = day_items
            .iter()
            .filter(|item| item.closed_at.is_some())
            .count() as i64;
        let overdue_runs = day_items
            .iter()
            .filter(|item| {
                item.closure_state == "overdue_open" || item.closure_state == "closed_late"
            })
            .count() as i64;
        timeline.push(RestoreEvidenceComplianceScorecardTimelinePoint {
            date: day.to_string(),
            required_runs,
            closed_runs,
            overdue_runs,
        });
        day += Duration::days(1);
    }

    let metrics = RestoreEvidenceComplianceMetrics {
        required_runs: items.len() as i64,
        closed_runs: items.iter().filter(|item| item.closed_at.is_some()).count() as i64,
        closed_within_sla_runs: items
            .iter()
            .filter(|item| item.closure_state == "closed_within_sla")
            .count() as i64,
        open_runs: items.iter().filter(|item| item.closed_at.is_none()).count() as i64,
        overdue_runs: items
            .iter()
            .filter(|item| {
                item.closure_state == "overdue_open" || item.closure_state == "closed_late"
            })
            .count() as i64,
        overdue_open_runs: items
            .iter()
            .filter(|item| item.closure_state == "overdue_open")
            .count() as i64,
    };

    let mut overdue_items = items
        .iter()
        .filter(|item| {
            item.closure_state == "overdue_open" || item.closure_state == "closed_late"
        })
        .cloned()
        .collect::<Vec<_>>();
    overdue_items.sort_by(|left, right| {
        right
            .overdue_hours
            .cmp(&left.overdue_hours)
            .then_with(|| right.started_at.cmp(&left.started_at))
            .then_with(|| left.run_id.cmp(&right.run_id))
    });

    Ok(RestoreEvidenceComplianceScorecardResponse {
        generated_at: as_of,
        scorecard_key: format!("restore-evidence-compliance-{}", week_start.format("%Y-%m-%d")),
        week_start: week_start.to_string(),
        week_end: week_end.to_string(),
        as_of,
        policy: build_restore_evidence_policy_view(&policy),
        metrics,
        timeline,
        overdue_items,
    })
}

fn build_restore_evidence_policy_view(
    policy: &RestoreEvidenceCompliancePolicy,
) -> RestoreEvidenceCompliancePolicyView {
    RestoreEvidenceCompliancePolicyView {
        policy_key: policy.policy_key.clone(),
        mode: policy.mode.clone(),
        sla_hours: policy.sla_hours,
        require_failed_runs: policy.require_failed_runs,
        require_drill_runs: policy.require_drill_runs,
        updated_by: policy.updated_by.clone(),
        updated_at: policy.updated_at,
    }
}

fn evaluate_restore_evidence_closure_state(
    deadline_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    as_of: DateTime<Utc>,
) -> (String, i64) {
    if let Some(closed_at) = closed_at {
        if closed_at <= deadline_at {
            return ("closed_within_sla".to_string(), 0);
        }
        let overdue_hours = (closed_at - deadline_at).num_hours().max(1);
        return ("closed_late".to_string(), overdue_hours);
    }

    if as_of > deadline_at {
        let overdue_hours = (as_of - deadline_at).num_hours().max(1);
        return ("overdue_open".to_string(), overdue_hours);
    }

    ("open_within_sla".to_string(), 0)
}

fn restore_evidence_scorecard_to_csv(
    scorecard: &RestoreEvidenceComplianceScorecardResponse,
) -> String {
    let mut lines = vec![
        "section,field,value".to_string(),
        format!(
            "meta,scorecard_key,{}",
            escape_csv_cell(scorecard.scorecard_key.as_str())
        ),
        format!(
            "meta,generated_at,{}",
            escape_csv_cell(scorecard.generated_at.to_rfc3339().as_str())
        ),
        format!(
            "meta,as_of,{}",
            escape_csv_cell(scorecard.as_of.to_rfc3339().as_str())
        ),
        format!(
            "meta,week_start,{}",
            escape_csv_cell(scorecard.week_start.as_str())
        ),
        format!(
            "meta,week_end,{}",
            escape_csv_cell(scorecard.week_end.as_str())
        ),
        format!(
            "policy,policy_key,{}",
            escape_csv_cell(scorecard.policy.policy_key.as_str())
        ),
        format!("policy,mode,{}", escape_csv_cell(scorecard.policy.mode.as_str())),
        format!("policy,sla_hours,{}", scorecard.policy.sla_hours),
        format!(
            "policy,require_failed_runs,{}",
            scorecard.policy.require_failed_runs
        ),
        format!(
            "policy,require_drill_runs,{}",
            scorecard.policy.require_drill_runs
        ),
        format!(
            "policy,updated_by,{}",
            escape_csv_cell(scorecard.policy.updated_by.as_str())
        ),
        format!(
            "policy,updated_at,{}",
            escape_csv_cell(scorecard.policy.updated_at.to_rfc3339().as_str())
        ),
        format!("metrics,required_runs,{}", scorecard.metrics.required_runs),
        format!("metrics,closed_runs,{}", scorecard.metrics.closed_runs),
        format!(
            "metrics,closed_within_sla_runs,{}",
            scorecard.metrics.closed_within_sla_runs
        ),
        format!("metrics,open_runs,{}", scorecard.metrics.open_runs),
        format!("metrics,overdue_runs,{}", scorecard.metrics.overdue_runs),
        format!(
            "metrics,overdue_open_runs,{}",
            scorecard.metrics.overdue_open_runs
        ),
        "timeline,date,required_runs,closed_runs,overdue_runs".to_string(),
    ];

    for point in &scorecard.timeline {
        lines.push(format!(
            "timeline,{},{},{},{}",
            escape_csv_cell(point.date.as_str()),
            point.required_runs,
            point.closed_runs,
            point.overdue_runs
        ));
    }

    lines.push("overdue_items,run_id,policy_id,run_type,run_status,started_at,deadline_at,closure_state,closed_at,evidence_total,closed_evidence_count,latest_evidence_id,latest_closure_status,latest_evidence_at,overdue_hours,run_ref".to_string());
    for item in &scorecard.overdue_items {
        lines.push(format!(
            "overdue_item,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            item.run_id,
            item.policy_id,
            escape_csv_cell(item.run_type.as_str()),
            escape_csv_cell(item.run_status.as_str()),
            escape_csv_cell(item.started_at.to_rfc3339().as_str()),
            escape_csv_cell(item.deadline_at.to_rfc3339().as_str()),
            escape_csv_cell(item.closure_state.as_str()),
            escape_csv_cell(item.closed_at.map(|value| value.to_rfc3339()).unwrap_or_default().as_str()),
            item.evidence_total,
            item.closed_evidence_count,
            escape_csv_cell(
                item.latest_evidence_id
                    .map(|value| value.to_string())
                    .unwrap_or_default()
                    .as_str(),
            ),
            escape_csv_cell(item.latest_closure_status.as_deref().unwrap_or("")),
            escape_csv_cell(
                item.latest_evidence_at
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default()
                    .as_str(),
            ),
            item.overdue_hours,
            escape_csv_cell(item.run_ref.as_str()),
        ));
    }

    lines.join("\n")
}

async fn load_or_init_restore_evidence_compliance_policy(
    db: &sqlx::PgPool,
) -> AppResult<RestoreEvidenceCompliancePolicy> {
    let existing: Option<RestoreEvidenceCompliancePolicyRow> = sqlx::query_as(
        "SELECT policy_key, mode, sla_hours, require_failed_runs, require_drill_runs, updated_by, updated_at
         FROM ops_restore_evidence_compliance_policies
         WHERE policy_key = $1
         LIMIT 1",
    )
    .bind(EVIDENCE_COMPLIANCE_POLICY_KEY)
    .fetch_optional(db)
    .await?;

    if let Some(row) = existing {
        return parse_restore_evidence_compliance_policy_row(row);
    }

    let inserted: RestoreEvidenceCompliancePolicyRow = sqlx::query_as(
        "INSERT INTO ops_restore_evidence_compliance_policies (
            policy_key, mode, sla_hours, require_failed_runs, require_drill_runs, updated_by, metadata
         )
         VALUES ($1, 'advisory', $2, TRUE, TRUE, 'system', $3)
         RETURNING policy_key, mode, sla_hours, require_failed_runs, require_drill_runs, updated_by, updated_at",
    )
    .bind(EVIDENCE_COMPLIANCE_POLICY_KEY)
    .bind(DEFAULT_EVIDENCE_COMPLIANCE_SLA_HOURS)
    .bind(json!({
        "initialized_by": "system",
        "policy_kind": "restore_evidence_sla"
    }))
    .fetch_one(db)
    .await?;

    parse_restore_evidence_compliance_policy_row(inserted)
}

fn parse_restore_evidence_compliance_policy_row(
    row: RestoreEvidenceCompliancePolicyRow,
) -> AppResult<RestoreEvidenceCompliancePolicy> {
    let mode = normalize_restore_evidence_policy_mode(row.mode)?;
    let sla_hours = normalize_restore_evidence_sla_hours(row.sla_hours)?;
    if !row.require_failed_runs && !row.require_drill_runs {
        return Err(AppError::Validation(
            "restore evidence policy requires at least one enabled selector".to_string(),
        ));
    }

    Ok(RestoreEvidenceCompliancePolicy {
        policy_key: row.policy_key,
        mode,
        sla_hours,
        require_failed_runs: row.require_failed_runs,
        require_drill_runs: row.require_drill_runs,
        updated_by: row.updated_by,
        updated_at: row.updated_at,
    })
}

fn run_requires_restore_evidence(
    policy: &RestoreEvidenceCompliancePolicy,
    run_type: &str,
    run_status: &str,
) -> bool {
    (policy.require_failed_runs && run_status == "failed")
        || (policy.require_drill_runs && run_type == "drill")
}

fn enforce_restore_evidence_compliance(
    policy: &RestoreEvidenceCompliancePolicy,
    run: &BackupPolicyRunRecord,
    close_evidence: bool,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if policy.mode != "enforced"
        || !run_requires_restore_evidence(policy, run.run_type.as_str(), run.status.as_str())
        || close_evidence
    {
        return Ok(());
    }

    let deadline_at = run.started_at + Duration::hours(policy.sla_hours as i64);
    if now > deadline_at {
        return Err(AppError::Validation(format!(
            "restore evidence for run #{} exceeded SLA deadline {}; close_evidence=true is required by policy '{}'",
            run.id,
            deadline_at.to_rfc3339(),
            policy.policy_key
        )));
    }

    Ok(())
}

fn parse_scorecard_week_start(value: Option<String>) -> AppResult<NaiveDate> {
    if let Some(value) = value {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(monday_of(Utc::now().date_naive()));
        }
        let parsed = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
            AppError::Validation("week_start must use YYYY-MM-DD format".to_string())
        })?;
        return Ok(monday_of(parsed));
    }
    Ok(monday_of(Utc::now().date_naive()))
}

fn parse_optional_as_of(value: Option<String>) -> AppResult<Option<DateTime<Utc>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = DateTime::parse_from_rfc3339(trimmed).map_err(|_| {
        AppError::Validation("as_of must use RFC3339 datetime format".to_string())
    })?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

fn monday_of(date: NaiveDate) -> NaiveDate {
    let weekday = date.weekday().number_from_monday() as i64;
    date - Duration::days((weekday - 1).max(0))
}

fn normalize_restore_evidence_policy_mode(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("mode is required".to_string()));
    }
    if normalized.len() > MAX_POLICY_MODE_LEN {
        return Err(AppError::Validation(format!(
            "mode length must be <= {MAX_POLICY_MODE_LEN}"
        )));
    }
    match normalized.as_str() {
        "advisory" | "enforced" => Ok(normalized),
        _ => Err(AppError::Validation(
            "mode must be one of: advisory, enforced".to_string(),
        )),
    }
}

fn normalize_restore_evidence_sla_hours(value: i32) -> AppResult<i32> {
    if !(1..=MAX_EVIDENCE_COMPLIANCE_SLA_HOURS).contains(&value) {
        return Err(AppError::Validation(format!(
            "sla_hours must be in [1, {MAX_EVIDENCE_COMPLIANCE_SLA_HOURS}]"
        )));
    }
    Ok(value)
}

fn append_backup_run_filters(
    builder: &mut QueryBuilder<Postgres>,
    policy_id: Option<i64>,
    run_type: Option<String>,
) {
    if let Some(policy_id) = policy_id {
        builder.push(" AND r.policy_id = ").push_bind(policy_id);
    }
    if let Some(run_type) = run_type {
        builder.push(" AND r.run_type = ").push_bind(run_type);
    }
}

fn append_restore_evidence_filters(
    builder: &mut QueryBuilder<Postgres>,
    policy_id: Option<i64>,
    run_status: Option<String>,
) {
    if let Some(policy_id) = policy_id {
        builder.push(" AND e.policy_id = ").push_bind(policy_id);
    }
    if let Some(run_status) = run_status {
        builder.push(" AND e.run_status = ").push_bind(run_status);
    }
}

async fn execute_backup_run(
    db: &sqlx::PgPool,
    policy_id: i64,
    run_type: &str,
    actor: &str,
    triggered_by_scheduler: bool,
    simulate_failure: bool,
    note: Option<String>,
) -> AppResult<(BackupPolicyRecord, BackupPolicyRunRecord, Vec<String>)> {
    let policy = load_backup_policy(db, policy_id).await?;
    let now = Utc::now();

    let status = if simulate_failure { "failed" } else { "succeeded" };
    let (error_message, remediation_hint, remediation_hints): (Option<String>, Option<String>, Vec<String>) =
        if simulate_failure {
            (
                Some(format!(
                    "simulated {} failure: verify destination connectivity and credentials",
                    run_type
                )),
                Some(
                    "Check destination reachability, permissions, and retention budget before retry."
                        .to_string(),
                ),
                vec![
                    "Validate destination endpoint and credentials.".to_string(),
                    "Confirm retention policy has enough free capacity.".to_string(),
                ],
            )
        } else {
            (
                None,
                Some("Run completed successfully; keep restore evidence for audit.".to_string()),
                vec![
                    "Keep latest backup and drill evidence linked in weekly digest.".to_string(),
                ],
            )
        };

    let run: BackupPolicyRunRecord = sqlx::query_as(
        "INSERT INTO ops_backup_policy_runs (
            policy_id, run_type, status, triggered_by, triggered_by_scheduler,
            note, remediation_hint, error_message, started_at, finished_at, metadata
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW(), $9)
         RETURNING id, policy_id, run_type, status, triggered_by, triggered_by_scheduler,
                   note, remediation_hint, error_message, started_at, finished_at,
                   0::bigint AS restore_evidence_count,
                   NULL::timestamptz AS latest_restore_verified_at,
                   NULL::text AS latest_restore_closure_status,
                   created_at",
    )
    .bind(policy_id)
    .bind(run_type)
    .bind(status)
    .bind(actor)
    .bind(triggered_by_scheduler)
    .bind(note.clone())
    .bind(remediation_hint.clone())
    .bind(error_message.clone())
    .bind(json!({
        "policy_key": policy.policy_key,
        "simulate_failure": simulate_failure,
    }))
    .fetch_one(db)
    .await?;

    let updated_policy: BackupPolicyRecord = if run_type == "backup" {
        let next_backup_at = compute_next_backup_at(
            policy.frequency.as_str(),
            policy.schedule_time_utc.as_str(),
            policy.schedule_weekday,
            now,
        )?;

        sqlx::query_as(
            "UPDATE ops_backup_policies
             SET last_backup_status = $2,
                 last_backup_at = NOW(),
                 last_backup_error = $3,
                 next_backup_at = $4,
                 updated_by = $5,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                       retention_days, destination_type, destination_uri, destination_validated,
                       drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                       last_backup_status, last_backup_at, last_backup_error,
                       last_drill_status, last_drill_at, last_drill_error,
                       next_backup_at, next_drill_at, updated_by, created_at, updated_at",
        )
        .bind(policy_id)
        .bind(status)
        .bind(error_message)
        .bind(next_backup_at)
        .bind(actor)
        .fetch_one(db)
        .await?
    } else {
        let next_drill_at = if policy.drill_enabled {
            Some(compute_next_drill_at(
                policy.drill_frequency.as_str(),
                policy.drill_time_utc.as_str(),
                policy.drill_weekday,
                now,
            )?)
        } else {
            None
        };

        sqlx::query_as(
            "UPDATE ops_backup_policies
             SET last_drill_status = $2,
                 last_drill_at = NOW(),
                 last_drill_error = $3,
                 next_drill_at = $4,
                 updated_by = $5,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                       retention_days, destination_type, destination_uri, destination_validated,
                       drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                       last_backup_status, last_backup_at, last_backup_error,
                       last_drill_status, last_drill_at, last_drill_error,
                       next_backup_at, next_drill_at, updated_by, created_at, updated_at",
        )
        .bind(policy_id)
        .bind(status)
        .bind(error_message)
        .bind(next_drill_at)
        .bind(actor)
        .fetch_one(db)
        .await?
    };

    Ok((updated_policy, run, remediation_hints))
}

async fn load_backup_policy(db: &sqlx::PgPool, id: i64) -> AppResult<BackupPolicyRecord> {
    if id <= 0 {
        return Err(AppError::Validation(
            "backup policy id must be a positive integer".to_string(),
        ));
    }

    let item: Option<BackupPolicyRecord> = sqlx::query_as(
        "SELECT id, policy_key, name, frequency, schedule_time_utc, schedule_weekday,
                retention_days, destination_type, destination_uri, destination_validated,
                drill_enabled, drill_frequency, drill_weekday, drill_time_utc,
                last_backup_status, last_backup_at, last_backup_error,
                last_drill_status, last_drill_at, last_drill_error,
                next_backup_at, next_drill_at, updated_by, created_at, updated_at
         FROM ops_backup_policies
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("backup policy {id} not found")))
}

async fn load_backup_run(db: &sqlx::PgPool, id: i64) -> AppResult<BackupPolicyRunRecord> {
    if id <= 0 {
        return Err(AppError::Validation(
            "backup run id must be a positive integer".to_string(),
        ));
    }

    let item: Option<BackupPolicyRunRecord> = sqlx::query_as(
        "SELECT id, policy_id, run_type, status, triggered_by, triggered_by_scheduler,
                note, remediation_hint, error_message, started_at, finished_at,
                (SELECT COUNT(*) FROM ops_backup_restore_evidence e WHERE e.run_id = r.id) AS restore_evidence_count,
                (SELECT e.created_at
                 FROM ops_backup_restore_evidence e
                 WHERE e.run_id = r.id
                 ORDER BY e.created_at DESC, e.id DESC
                 LIMIT 1) AS latest_restore_verified_at,
                (SELECT e.closure_status
                 FROM ops_backup_restore_evidence e
                 WHERE e.run_id = r.id
                 ORDER BY e.created_at DESC, e.id DESC
                 LIMIT 1) AS latest_restore_closure_status,
                created_at
         FROM ops_backup_policy_runs r
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("backup run {id} not found")))
}

async fn load_restore_evidence(db: &sqlx::PgPool, id: i64) -> AppResult<BackupRestoreEvidenceRecord> {
    if id <= 0 {
        return Err(AppError::Validation(
            "restore evidence id must be a positive integer".to_string(),
        ));
    }

    let item: Option<BackupRestoreEvidenceRecord> = sqlx::query_as(
        "SELECT id, run_id, policy_id, run_type, run_status, ticket_ref, artifact_url, note,
                verifier, closure_status, closed_at, closed_by, created_at, updated_at
         FROM ops_backup_restore_evidence
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("restore evidence {id} not found")))
}

fn normalize_policy_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("policy_key is required".to_string()));
    }
    if normalized.len() > MAX_POLICY_KEY_LEN {
        return Err(AppError::Validation(format!(
            "policy_key length must be <= {MAX_POLICY_KEY_LEN}"
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(AppError::Validation(
            "policy_key must only contain lowercase letters, digits, or '-'".to_string(),
        ));
    }
    Ok(normalized)
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

fn normalize_backup_frequency(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "daily" | "weekly" => Ok(normalized),
        _ => Err(AppError::Validation(
            "frequency must be one of: daily, weekly".to_string(),
        )),
    }
}

fn normalize_drill_frequency(value: Option<String>) -> AppResult<String> {
    let normalized = value
        .unwrap_or_else(|| "weekly".to_string())
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "weekly" | "monthly" | "quarterly" => Ok(normalized),
        _ => Err(AppError::Validation(
            "drill_frequency must be one of: weekly, monthly, quarterly".to_string(),
        )),
    }
}

fn normalize_destination_type(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "s3" | "nfs" | "local" => Ok(normalized),
        _ => Err(AppError::Validation(
            "destination_type must be one of: s3, nfs, local".to_string(),
        )),
    }
}

fn normalize_destination_uri(value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("destination_uri is required".to_string()));
    }
    if trimmed.len() > MAX_DESTINATION_URI_LEN {
        return Err(AppError::Validation(format!(
            "destination_uri length must be <= {MAX_DESTINATION_URI_LEN}"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_retention_days(value: i32) -> AppResult<i32> {
    if !(1..=3650).contains(&value) {
        return Err(AppError::Validation(
            "retention_days must be in [1, 3650]".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_schedule_weekday(value: Option<i16>, frequency: &str) -> AppResult<Option<i16>> {
    if frequency != "weekly" {
        return Ok(None);
    }
    let day = value.unwrap_or(1);
    if !(1..=7).contains(&day) {
        return Err(AppError::Validation(
            "schedule_weekday must be in [1, 7] where Monday=1".to_string(),
        ));
    }
    Ok(Some(day))
}

fn normalize_drill_weekday(value: Option<i16>, frequency: &str) -> AppResult<Option<i16>> {
    if frequency != "weekly" {
        return Ok(None);
    }
    let day = value.unwrap_or(3);
    if !(1..=7).contains(&day) {
        return Err(AppError::Validation(
            "drill_weekday must be in [1, 7] where Monday=1".to_string(),
        ));
    }
    Ok(Some(day))
}

fn normalize_hhmm(value: String, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let parsed = NaiveTime::parse_from_str(trimmed, "%H:%M")
        .map_err(|_| AppError::Validation(format!("{field} must use HH:MM 24h format")))?;
    Ok(format!("{:02}:{:02}", parsed.hour(), parsed.minute()))
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() > max_len {
            Some(trimmed[..max_len].to_string())
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn normalize_optional_note(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_NOTE_LEN {
        return Err(AppError::Validation(format!(
            "note length must be <= {MAX_NOTE_LEN}"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn validate_destination_uri(destination_type: &str, destination_uri: &str) -> AppResult<()> {
    let valid = match destination_type {
        "s3" => destination_uri.starts_with("s3://"),
        "nfs" => destination_uri.starts_with("nfs://"),
        "local" => destination_uri.starts_with('/') || destination_uri.starts_with("file://"),
        _ => false,
    };

    if !valid {
        return Err(AppError::Validation(format!(
            "destination_uri '{}' is invalid for destination_type '{}'",
            destination_uri, destination_type
        )));
    }
    Ok(())
}

fn normalize_run_type(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "backup" | "drill" => Ok(normalized),
        _ => Err(AppError::Validation(
            "run_type must be one of: backup, drill".to_string(),
        )),
    }
}

fn normalize_run_status(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "succeeded" | "failed" => Ok(normalized),
        _ => Err(AppError::Validation(
            "run_status must be one of: succeeded, failed".to_string(),
        )),
    }
}

fn compute_next_backup_at(
    frequency: &str,
    schedule_time_utc: &str,
    schedule_weekday: Option<i16>,
    now: DateTime<Utc>,
) -> AppResult<DateTime<Utc>> {
    let time = NaiveTime::parse_from_str(schedule_time_utc, "%H:%M")
        .map_err(|_| AppError::Validation("schedule_time_utc must use HH:MM format".to_string()))?;

    if frequency == "daily" {
        let today = now.date_naive();
        let today_dt = Utc.from_utc_datetime(&today.and_time(time));
        if today_dt > now {
            return Ok(today_dt);
        }
        return Ok(Utc.from_utc_datetime(&(today + Duration::days(1)).and_time(time)));
    }

    if frequency == "weekly" {
        let target_weekday = schedule_weekday.unwrap_or(1);
        let current_weekday = now.weekday().number_from_monday() as i16;
        let mut day_offset = (target_weekday - current_weekday + 7) % 7;
        let today = now.date_naive();
        let candidate = Utc.from_utc_datetime(&(today + Duration::days(day_offset as i64)).and_time(time));
        if day_offset == 0 && candidate <= now {
            day_offset = 7;
        }
        return Ok(Utc.from_utc_datetime(&(today + Duration::days(day_offset as i64)).and_time(time)));
    }

    Err(AppError::Validation(
        "frequency must be one of: daily, weekly".to_string(),
    ))
}

fn compute_next_drill_at(
    drill_frequency: &str,
    drill_time_utc: &str,
    drill_weekday: Option<i16>,
    now: DateTime<Utc>,
) -> AppResult<DateTime<Utc>> {
    let time = NaiveTime::parse_from_str(drill_time_utc, "%H:%M")
        .map_err(|_| AppError::Validation("drill_time_utc must use HH:MM format".to_string()))?;
    let today = now.date_naive();

    match drill_frequency {
        "weekly" => {
            let target_weekday = drill_weekday.unwrap_or(3);
            let current_weekday = now.weekday().number_from_monday() as i16;
            let mut day_offset = (target_weekday - current_weekday + 7) % 7;
            let candidate = Utc.from_utc_datetime(&(today + Duration::days(day_offset as i64)).and_time(time));
            if day_offset == 0 && candidate <= now {
                day_offset = 7;
            }
            Ok(Utc.from_utc_datetime(&(today + Duration::days(day_offset as i64)).and_time(time)))
        }
        "monthly" => {
            let candidate = Utc.from_utc_datetime(&(today + Duration::days(30)).and_time(time));
            Ok(candidate)
        }
        "quarterly" => {
            let candidate = Utc.from_utc_datetime(&(today + Duration::days(90)).and_time(time));
            Ok(candidate)
        }
        _ => Err(AppError::Validation(
            "drill_frequency must be one of: weekly, monthly, quarterly".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Duration, TimeZone, Utc};

    use super::{
        compute_next_backup_at, compute_next_drill_at, normalize_destination_type,
        normalize_run_type, normalize_schedule_weekday, validate_destination_uri,
    };

    #[test]
    fn validates_destination_type_and_uri() {
        assert_eq!(normalize_destination_type("S3".to_string()).expect("type"), "s3");
        assert!(validate_destination_uri("s3", "s3://bucket/path").is_ok());
        assert!(validate_destination_uri("nfs", "s3://bucket").is_err());
    }

    #[test]
    fn computes_next_daily_and_weekly_schedule() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 1, 0, 0).single().expect("now");
        let daily = compute_next_backup_at("daily", "02:00", None, now).expect("daily");
        assert_eq!(daily, Utc.with_ymd_and_hms(2026, 3, 6, 2, 0, 0).single().expect("daily dt"));

        let weekly = compute_next_backup_at("weekly", "02:00", Some(7), now).expect("weekly");
        assert_eq!(weekly.weekday().number_from_monday(), 7);
    }

    #[test]
    fn computes_next_drill_schedule() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 1, 0, 0).single().expect("now");
        let weekly = compute_next_drill_at("weekly", "03:00", Some(7), now).expect("weekly");
        assert_eq!(weekly.weekday().number_from_monday(), 7);

        let monthly = compute_next_drill_at("monthly", "03:00", None, now).expect("monthly");
        assert!(monthly > now + Duration::days(29));
    }

    #[test]
    fn validates_run_type_and_weekday() {
        assert_eq!(normalize_run_type("backup".to_string()).expect("run"), "backup");
        assert!(normalize_run_type("invalid".to_string()).is_err());
        assert!(normalize_schedule_weekday(Some(8), "weekly").is_err());
    }
}
