use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/weekly-digest", get(get_weekly_digest))
        .route("/cockpit/weekly-digest/export", get(export_weekly_digest))
        .route("/cockpit/handover-digest", get(get_handover_digest))
        .route(
            "/cockpit/handover-digest/export",
            get(export_handover_digest),
        )
        .route(
            "/cockpit/handover-digest/items/{item_key}/close",
            post(close_handover_item),
        )
        .route(
            "/cockpit/handover-digest/reminders",
            get(get_handover_reminders),
        )
        .route(
            "/cockpit/handover-digest/reminders/export",
            get(export_handover_reminders),
        )
        .route("/cockpit/handover-readiness", get(get_handover_readiness))
}

#[derive(Debug, Deserialize, Default)]
struct WeeklyDigestQuery {
    week_start: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WeeklyDigestExportQuery {
    week_start: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct WeeklyDigestMetrics {
    open_critical_alerts: i64,
    open_warning_alerts: i64,
    suppressed_alert_threads: i64,
    stale_open_tickets: i64,
    workflow_approval_backlog: i64,
    playbook_approval_backlog: i64,
    backup_failed_policies: i64,
    drill_failed_policies: i64,
    continuity_runs_requiring_evidence: i64,
    continuity_runs_with_evidence: i64,
    continuity_runs_missing_evidence: i64,
    locked_local_accounts: i64,
    local_accounts_without_mfa: i64,
}

#[derive(Debug, Serialize, Clone)]
struct WeeklyDigestResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    week_start: String,
    week_end: String,
    metrics: WeeklyDigestMetrics,
    top_risks: Vec<String>,
    unresolved_items: Vec<String>,
    recommended_actions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WeeklyDigestExportResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    format: String,
    content: String,
}

#[derive(Debug, Deserialize, Default)]
struct HandoverDigestQuery {
    shift_date: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct HandoverDigestExportQuery {
    shift_date: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct HandoverReminderQuery {
    shift_date: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct HandoverReminderExportQuery {
    shift_date: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CloseHandoverItemRequest {
    shift_date: Option<String>,
    source_type: String,
    source_id: i64,
    next_owner: String,
    next_action: String,
    note: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct HandoverDigestMetrics {
    unresolved_incidents: i64,
    escalation_backlog: i64,
    failed_continuity_runs: i64,
    pending_approvals: i64,
    restore_evidence_missing_runs: i64,
    closed_items: i64,
    overdue_open_items: i64,
    ownership_gap_items: i64,
}

#[derive(Debug, Serialize, Clone)]
struct HandoverCarryoverItem {
    item_key: String,
    source_type: String,
    source_id: i64,
    title: String,
    owner: String,
    next_owner: String,
    next_action: String,
    status: String,
    note: Option<String>,
    risk_level: String,
    observed_at: DateTime<Utc>,
    source_ref: String,
    overdue: bool,
    overdue_days: i64,
    ownership_violations: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
struct HandoverOverdueTrendPoint {
    shift_date: String,
    open_items: i64,
    overdue_items: i64,
}

#[derive(Debug, Serialize, Clone)]
struct HandoverDigestResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    shift_date: String,
    metrics: HandoverDigestMetrics,
    overdue_trend: Vec<HandoverOverdueTrendPoint>,
    items: Vec<HandoverCarryoverItem>,
}

#[derive(Debug, Serialize)]
struct HandoverDigestExportResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    format: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct HandoverReminderResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    shift_date: String,
    total: usize,
    items: Vec<HandoverCarryoverItem>,
}

#[derive(Debug, Serialize)]
struct HandoverReminderExportResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    format: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct HandoverReadinessResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    shift_date: String,
    readiness_state: String,
    summary: HandoverReadinessSummary,
    reasons: Vec<String>,
    items: Vec<HandoverReadinessItem>,
}

#[derive(Debug, Serialize, Default)]
struct HandoverReadinessSummary {
    total: usize,
    ready: usize,
    at_risk: usize,
    blocking: usize,
    open_items: usize,
    closed_items: usize,
}

#[derive(Debug, Serialize)]
struct HandoverReadinessItem {
    item_key: String,
    source_type: String,
    title: String,
    owner: String,
    next_owner: String,
    next_action: String,
    status: String,
    risk_level: String,
    readiness_state: String,
    priority_score: i32,
    reason: String,
    observed_at: DateTime<Utc>,
    evidence_timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct HandoverItemUpdateRecord {
    id: i64,
    shift_date: NaiveDate,
    item_key: String,
    source_type: String,
    source_id: i64,
    status: String,
    next_owner: String,
    next_action: String,
    note: Option<String>,
    updated_by: String,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct IncidentCarryoverRow {
    alert_id: i64,
    title: String,
    severity: String,
    command_status: String,
    command_owner: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct TicketCarryoverRow {
    id: i64,
    ticket_no: String,
    title: String,
    priority: String,
    assignee: Option<String>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ContinuityRunCarryoverRow {
    id: i64,
    policy_id: i64,
    run_type: String,
    status: String,
    triggered_by: String,
    started_at: DateTime<Utc>,
    evidence_count: i64,
}

#[derive(Debug, FromRow)]
struct WorkflowApprovalCarryoverRow {
    id: i64,
    title: String,
    requester: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PlaybookApprovalCarryoverRow {
    id: i64,
    playbook_key: String,
    requester: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct HandoverTrendRow {
    shift_date: NaiveDate,
    open_items: i64,
}

async fn get_weekly_digest(
    State(state): State<AppState>,
    Query(query): Query<WeeklyDigestQuery>,
) -> AppResult<Json<WeeklyDigestResponse>> {
    let digest = build_weekly_digest(&state, query.week_start).await?;
    Ok(Json(digest))
}

async fn export_weekly_digest(
    State(state): State<AppState>,
    Query(query): Query<WeeklyDigestExportQuery>,
) -> AppResult<Json<WeeklyDigestExportResponse>> {
    let digest = build_weekly_digest(&state, query.week_start).await?;
    let format = query
        .format
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_ascii_lowercase();

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&digest).map_err(|err| {
            AppError::Validation(format!("failed to serialize digest json: {err}"))
        })?,
        "csv" => digest_to_csv(&digest),
        _ => {
            return Err(AppError::Validation(
                "format must be one of: csv, json".to_string(),
            ));
        }
    };

    Ok(Json(WeeklyDigestExportResponse {
        generated_at: digest.generated_at,
        digest_key: digest.digest_key,
        format,
        content,
    }))
}

async fn get_handover_digest(
    State(state): State<AppState>,
    Query(query): Query<HandoverDigestQuery>,
) -> AppResult<Json<HandoverDigestResponse>> {
    let digest = build_handover_digest(&state, query.shift_date).await?;
    Ok(Json(digest))
}

async fn export_handover_digest(
    State(state): State<AppState>,
    Query(query): Query<HandoverDigestExportQuery>,
) -> AppResult<Json<HandoverDigestExportResponse>> {
    let digest = build_handover_digest(&state, query.shift_date).await?;
    let format = query
        .format
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_ascii_lowercase();

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&digest).map_err(|err| {
            AppError::Validation(format!("failed to serialize handover digest json: {err}"))
        })?,
        "csv" => handover_digest_to_csv(&digest),
        _ => {
            return Err(AppError::Validation(
                "format must be one of: csv, json".to_string(),
            ));
        }
    };

    Ok(Json(HandoverDigestExportResponse {
        generated_at: digest.generated_at,
        digest_key: digest.digest_key,
        format,
        content,
    }))
}

async fn get_handover_reminders(
    State(state): State<AppState>,
    Query(query): Query<HandoverReminderQuery>,
) -> AppResult<Json<HandoverReminderResponse>> {
    let digest = build_handover_digest(&state, query.shift_date).await?;
    let items = digest
        .items
        .iter()
        .filter(|item| {
            item.status == "open" && (item.overdue || !item.ownership_violations.is_empty())
        })
        .cloned()
        .collect::<Vec<_>>();

    Ok(Json(HandoverReminderResponse {
        generated_at: digest.generated_at,
        digest_key: digest.digest_key,
        shift_date: digest.shift_date,
        total: items.len(),
        items,
    }))
}

async fn get_handover_readiness(
    State(state): State<AppState>,
    Query(query): Query<HandoverDigestQuery>,
) -> AppResult<Json<HandoverReadinessResponse>> {
    let digest = build_handover_digest(&state, query.shift_date).await?;
    let readiness = build_handover_readiness_response(&digest);
    Ok(Json(readiness))
}

async fn export_handover_reminders(
    State(state): State<AppState>,
    Query(query): Query<HandoverReminderExportQuery>,
) -> AppResult<Json<HandoverReminderExportResponse>> {
    let reminder = get_handover_reminders(
        State(state),
        Query(HandoverReminderQuery {
            shift_date: query.shift_date,
        }),
    )
    .await?
    .0;
    let format = query
        .format
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_ascii_lowercase();

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&reminder).map_err(|err| {
            AppError::Validation(format!("failed to serialize handover reminder json: {err}"))
        })?,
        "csv" => handover_reminder_to_csv(&reminder),
        _ => {
            return Err(AppError::Validation(
                "format must be one of: csv, json".to_string(),
            ));
        }
    };

    Ok(Json(HandoverReminderExportResponse {
        generated_at: reminder.generated_at,
        digest_key: reminder.digest_key,
        format,
        content,
    }))
}

async fn close_handover_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_key): Path<String>,
    Json(payload): Json<CloseHandoverItemRequest>,
) -> AppResult<Json<HandoverItemUpdateRecord>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let shift_date = parse_shift_date(payload.shift_date)?;
    let item_key = required_trimmed("item_key", item_key, 128)?;
    let source_type = required_trimmed("source_type", payload.source_type, 32)?;
    if payload.source_id <= 0 {
        return Err(AppError::Validation(
            "source_id must be a positive integer".to_string(),
        ));
    }
    let next_owner = required_trimmed("next_owner", payload.next_owner, 128)?;
    let next_action = required_trimmed("next_action", payload.next_action, 1024)?;
    let note = trim_optional(payload.note, 1024);

    let item: HandoverItemUpdateRecord = sqlx::query_as(
        "INSERT INTO ops_handover_item_updates (
            shift_date, item_key, source_type, source_id,
            status, next_owner, next_action, note, updated_by, closed_at
         )
         VALUES ($1, $2, $3, $4, 'closed', $5, $6, $7, $8, NOW())
         ON CONFLICT (shift_date, item_key)
         DO UPDATE SET
            source_type = EXCLUDED.source_type,
            source_id = EXCLUDED.source_id,
            status = 'closed',
            next_owner = EXCLUDED.next_owner,
            next_action = EXCLUDED.next_action,
            note = EXCLUDED.note,
            updated_by = EXCLUDED.updated_by,
            closed_at = NOW(),
            updated_at = NOW()
         RETURNING id, shift_date, item_key, source_type, source_id, status, next_owner,
                   next_action, note, updated_by, closed_at, created_at, updated_at",
    )
    .bind(shift_date)
    .bind(item_key.clone())
    .bind(source_type.clone())
    .bind(payload.source_id)
    .bind(next_owner.clone())
    .bind(next_action.clone())
    .bind(note.clone())
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.handover.item.close".to_string(),
            target_type: "ops_handover_item".to_string(),
            target_id: Some(item.item_key.clone()),
            result: "success".to_string(),
            message: note,
            metadata: serde_json::json!({
                "shift_date": item.shift_date.to_string(),
                "source_type": source_type,
                "source_id": payload.source_id,
                "next_owner": next_owner,
                "next_action": next_action,
                "status": item.status,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn build_weekly_digest(
    state: &AppState,
    week_start_raw: Option<String>,
) -> AppResult<WeeklyDigestResponse> {
    let week_start = parse_week_start(week_start_raw)?;
    let week_end = week_start + Duration::days(6);
    let window_end_exclusive = week_end + Duration::days(1);

    let open_critical_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM unified_alerts
         WHERE status IN ('open', 'acknowledged')
           AND severity = 'critical'",
    )
    .fetch_one(&state.db)
    .await?;

    let open_warning_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM unified_alerts
         WHERE status IN ('open', 'acknowledged')
           AND severity = 'warning'",
    )
    .fetch_one(&state.db)
    .await?;

    let suppressed_alert_threads: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT alert_id)
         FROM alert_policy_actions
         WHERE action = 'suppressed'
           AND created_at >= $1
           AND created_at < $2",
    )
    .bind(week_start)
    .bind(window_end_exclusive)
    .fetch_one(&state.db)
    .await?;

    let stale_open_tickets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM tickets
         WHERE status IN ('open', 'in_progress')
           AND created_at < NOW() - INTERVAL '24 hour'",
    )
    .fetch_one(&state.db)
    .await?;

    let workflow_approval_backlog: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_requests
         WHERE status = 'pending_approval'",
    )
    .fetch_one(&state.db)
    .await?;

    let playbook_approval_backlog: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at > NOW()",
    )
    .fetch_one(&state.db)
    .await?;

    let backup_failed_policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policies
         WHERE last_backup_status = 'failed'",
    )
    .fetch_one(&state.db)
    .await?;

    let drill_failed_policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policies
         WHERE drill_enabled = TRUE
           AND last_drill_status = 'failed'",
    )
    .fetch_one(&state.db)
    .await?;

    let continuity_runs_requiring_evidence: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policy_runs
         WHERE status = 'failed'
            OR run_type = 'drill'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let continuity_runs_with_evidence: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT e.run_id)
         FROM ops_backup_restore_evidence e
         INNER JOIN ops_backup_policy_runs r ON r.id = e.run_id
         WHERE r.status = 'failed'
            OR r.run_type = 'drill'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let continuity_runs_missing_evidence =
        (continuity_runs_requiring_evidence - continuity_runs_with_evidence).max(0);

    let locked_local_accounts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM auth_local_credentials
         WHERE locked_until IS NOT NULL
           AND locked_until > NOW()",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let local_accounts_without_mfa: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM auth_local_credentials
         WHERE mfa_enabled = FALSE",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let metrics = WeeklyDigestMetrics {
        open_critical_alerts,
        open_warning_alerts,
        suppressed_alert_threads,
        stale_open_tickets,
        workflow_approval_backlog,
        playbook_approval_backlog,
        backup_failed_policies,
        drill_failed_policies,
        continuity_runs_requiring_evidence,
        continuity_runs_with_evidence,
        continuity_runs_missing_evidence,
        locked_local_accounts,
        local_accounts_without_mfa,
    };

    let mut top_risks = Vec::new();
    if open_critical_alerts > 0 {
        top_risks.push(format!(
            "{open_critical_alerts} critical alerts remain open/acknowledged."
        ));
    }
    if backup_failed_policies > 0 {
        top_risks.push(format!(
            "{backup_failed_policies} backup policies report latest failure."
        ));
    }
    if drill_failed_policies > 0 {
        top_risks.push(format!(
            "{drill_failed_policies} drill policies report latest failure."
        ));
    }
    if continuity_runs_missing_evidence > 0 {
        top_risks.push(format!(
            "{continuity_runs_missing_evidence} continuity runs are missing restore verification evidence."
        ));
    }
    if locked_local_accounts > 0 {
        top_risks.push(format!(
            "{locked_local_accounts} local accounts are currently locked."
        ));
    }
    if top_risks.is_empty() {
        top_risks.push("No critical blocker detected in weekly digest snapshot.".to_string());
    }

    let mut unresolved_items = Vec::new();
    if stale_open_tickets > 0 {
        unresolved_items.push(format!(
            "{stale_open_tickets} open tickets have exceeded 24h without closure."
        ));
    }
    if workflow_approval_backlog > 0 || playbook_approval_backlog > 0 {
        unresolved_items.push(format!(
            "Approval backlog: workflow={}, playbook={}",
            workflow_approval_backlog, playbook_approval_backlog
        ));
    }
    if suppressed_alert_threads > 0 {
        unresolved_items.push(format!(
            "{suppressed_alert_threads} alert threads were suppressed this week; validate no incident is hidden."
        ));
    }
    if continuity_runs_missing_evidence > 0 {
        unresolved_items.push(format!(
            "Restore evidence gap: {continuity_runs_missing_evidence} required runs have no verification artifact."
        ));
    }
    if unresolved_items.is_empty() {
        unresolved_items.push("No unresolved item above digest threshold.".to_string());
    }

    let mut recommended_actions = Vec::new();
    if open_critical_alerts > 0 {
        recommended_actions.push(
            "Escalate critical alerts and confirm ownership in ticket queue today.".to_string(),
        );
    }
    if backup_failed_policies > 0 || drill_failed_policies > 0 {
        recommended_actions.push(
            "Run backup/drill manually after destination validation and attach remediation evidence.".to_string(),
        );
    }
    if continuity_runs_missing_evidence > 0 {
        recommended_actions.push(
            "Attach restore verification ticket/artifact evidence and close continuity evidence records.".to_string(),
        );
    }
    if local_accounts_without_mfa > 0 {
        recommended_actions.push(format!(
            "Review {local_accounts_without_mfa} local accounts without MFA and enforce enrollment policy."
        ));
    }
    if workflow_approval_backlog > 0 || playbook_approval_backlog > 0 {
        recommended_actions
            .push("Clear approval queue to reduce high-risk remediation lead time.".to_string());
    }
    if recommended_actions.is_empty() {
        recommended_actions.push(
            "Keep current cadence and rerun digest next week for trend comparison.".to_string(),
        );
    }

    let digest_key = format!("weekly-{}", week_start.format("%Y-%m-%d"));

    Ok(WeeklyDigestResponse {
        generated_at: Utc::now(),
        digest_key,
        week_start: week_start.format("%Y-%m-%d").to_string(),
        week_end: week_end.format("%Y-%m-%d").to_string(),
        metrics,
        top_risks,
        unresolved_items,
        recommended_actions,
    })
}

async fn build_handover_digest(
    state: &AppState,
    shift_date_raw: Option<String>,
) -> AppResult<HandoverDigestResponse> {
    let shift_date = parse_shift_date(shift_date_raw)?;
    let update_rows: Vec<HandoverItemUpdateRecord> = sqlx::query_as(
        "SELECT id, shift_date, item_key, source_type, source_id, status, next_owner, next_action,
                note, updated_by, closed_at, created_at, updated_at
         FROM ops_handover_item_updates
         WHERE shift_date = $1
         ORDER BY updated_at DESC, id DESC",
    )
    .bind(shift_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let updates = update_rows
        .into_iter()
        .map(|item| (item.item_key.clone(), item))
        .collect::<HashMap<_, _>>();

    let incidents: Vec<IncidentCarryoverRow> = sqlx::query_as(
        "SELECT c.alert_id,
                COALESCE(a.title, concat(c.alert_id::text, ':incident')) AS title,
                COALESCE(a.severity, 'warning') AS severity,
                c.command_status,
                c.command_owner,
                c.updated_at
         FROM ops_incident_commands c
         LEFT JOIN unified_alerts a ON a.id = c.alert_id
         WHERE c.command_status IN ('triage', 'in_progress', 'blocked')
         ORDER BY c.updated_at DESC, c.alert_id DESC
         LIMIT 120",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let ticket_backlog: Vec<TicketCarryoverRow> = sqlx::query_as(
        "SELECT id, ticket_no, title, priority, assignee, updated_at
         FROM tickets
         WHERE status IN ('open', 'in_progress')
         ORDER BY created_at ASC, id ASC
         LIMIT 160",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let continuity_runs: Vec<ContinuityRunCarryoverRow> = sqlx::query_as(
        "SELECT r.id, r.policy_id, r.run_type, r.status, r.triggered_by, r.started_at,
                (SELECT COUNT(*) FROM ops_backup_restore_evidence e WHERE e.run_id = r.id) AS evidence_count
         FROM ops_backup_policy_runs r
         WHERE r.status = 'failed'
            OR r.run_type = 'drill'
         ORDER BY r.started_at DESC, r.id DESC
         LIMIT 160",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let workflow_approvals: Vec<WorkflowApprovalCarryoverRow> = sqlx::query_as(
        "SELECT id, title, requester, created_at
         FROM workflow_requests
         WHERE status = 'pending_approval'
         ORDER BY created_at ASC, id ASC
         LIMIT 120",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let playbook_approvals: Vec<PlaybookApprovalCarryoverRow> = sqlx::query_as(
        "SELECT id, playbook_key, requester, expires_at, created_at
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at > NOW()
         ORDER BY created_at ASC, id ASC
         LIMIT 120",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut items: Vec<HandoverCarryoverItem> = Vec::new();

    for row in &incidents {
        let item_key = format!("incident:{}", row.alert_id);
        let (status, next_owner, next_action, note) = resolve_handover_item_state(
            updates.get(&item_key),
            row.command_owner.clone(),
            format!(
                "Continue incident command from status '{}' and update ETA/blocker.",
                row.command_status
            ),
        );
        let risk_level = if row.command_status == "blocked" || row.severity == "critical" {
            "critical"
        } else {
            "high"
        };
        items.push(HandoverCarryoverItem {
            item_key,
            source_type: "incident_command".to_string(),
            source_id: row.alert_id,
            title: format!("[{}] {}", row.severity, row.title),
            owner: row.command_owner.clone(),
            next_owner,
            next_action,
            status,
            note,
            risk_level: risk_level.to_string(),
            observed_at: row.updated_at,
            source_ref: format!("/api/v1/ops/cockpit/incidents/{}", row.alert_id),
            overdue: false,
            overdue_days: 0,
            ownership_violations: Vec::new(),
        });
    }

    for row in &ticket_backlog {
        let item_key = format!("ticket:{}", row.id);
        let owner = row
            .assignee
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ops-oncall".to_string());
        let (status, next_owner, next_action, note) = resolve_handover_item_state(
            updates.get(&item_key),
            owner.clone(),
            format!(
                "Review ticket {} ({}) and decide escalation or closure.",
                row.ticket_no, row.priority
            ),
        );
        let risk_level = match row.priority.as_str() {
            "critical" => "critical",
            "high" => "high",
            _ => "medium",
        };
        items.push(HandoverCarryoverItem {
            item_key,
            source_type: "ticket_backlog".to_string(),
            source_id: row.id,
            title: format!("{}: {}", row.ticket_no, row.title),
            owner,
            next_owner,
            next_action,
            status,
            note,
            risk_level: risk_level.to_string(),
            observed_at: row.updated_at,
            source_ref: format!("/api/v1/tickets/{}", row.id),
            overdue: false,
            overdue_days: 0,
            ownership_violations: Vec::new(),
        });
    }

    for row in &continuity_runs {
        let item_key = format!("continuity_run:{}", row.id);
        let owner = row.triggered_by.clone();
        let default_action = if row.evidence_count == 0 {
            format!(
                "Attach restore verification evidence for {} run #{} before handover closure.",
                row.run_type, row.id
            )
        } else {
            format!(
                "Review {} run #{} output and handoff remediation plan.",
                row.run_type, row.id
            )
        };
        let (status, next_owner, next_action, note) =
            resolve_handover_item_state(updates.get(&item_key), owner.clone(), default_action);
        let risk_level = if row.status == "failed" || row.evidence_count == 0 {
            "critical"
        } else {
            "high"
        };
        items.push(HandoverCarryoverItem {
            item_key,
            source_type: "continuity_run".to_string(),
            source_id: row.id,
            title: format!(
                "run #{} policy #{} ({}/{}) evidence_count={}",
                row.id, row.policy_id, row.run_type, row.status, row.evidence_count
            ),
            owner,
            next_owner,
            next_action,
            status,
            note,
            risk_level: risk_level.to_string(),
            observed_at: row.started_at,
            source_ref: "/api/v1/ops/cockpit/backup/runs".to_string(),
            overdue: false,
            overdue_days: 0,
            ownership_violations: Vec::new(),
        });
    }

    for row in &workflow_approvals {
        let item_key = format!("workflow_approval:{}", row.id);
        let (status, next_owner, next_action, note) = resolve_handover_item_state(
            updates.get(&item_key),
            row.requester.clone(),
            "Assign approver and resolve workflow pending approval.".to_string(),
        );
        items.push(HandoverCarryoverItem {
            item_key,
            source_type: "workflow_approval".to_string(),
            source_id: row.id,
            title: row.title.clone(),
            owner: row.requester.clone(),
            next_owner,
            next_action,
            status,
            note,
            risk_level: "medium".to_string(),
            observed_at: row.created_at,
            source_ref: "/api/v1/workflow/requests".to_string(),
            overdue: false,
            overdue_days: 0,
            ownership_violations: Vec::new(),
        });
    }

    for row in &playbook_approvals {
        let item_key = format!("playbook_approval:{}", row.id);
        let (status, next_owner, next_action, note) = resolve_handover_item_state(
            updates.get(&item_key),
            row.requester.clone(),
            format!(
                "Resolve playbook approval '{}' before {}.",
                row.playbook_key,
                row.expires_at.to_rfc3339()
            ),
        );
        items.push(HandoverCarryoverItem {
            item_key,
            source_type: "playbook_approval".to_string(),
            source_id: row.id,
            title: format!("{} pending approval", row.playbook_key),
            owner: row.requester.clone(),
            next_owner,
            next_action,
            status,
            note,
            risk_level: "high".to_string(),
            observed_at: row.created_at,
            source_ref: "/api/v1/workflow/playbooks/approvals".to_string(),
            overdue: false,
            overdue_days: 0,
            ownership_violations: Vec::new(),
        });
    }

    items.sort_by(|left, right| {
        risk_rank(right.risk_level.as_str())
            .cmp(&risk_rank(left.risk_level.as_str()))
            .then_with(|| left.observed_at.cmp(&right.observed_at))
            .then_with(|| left.item_key.cmp(&right.item_key))
    });

    let shift_age_days = (Utc::now().date_naive() - shift_date).num_days().max(0);
    for item in &mut items {
        if item.status != "open" {
            item.overdue = false;
            item.overdue_days = 0;
            item.ownership_violations = Vec::new();
            continue;
        }
        let overdue_threshold_days = overdue_threshold_days_by_risk(item.risk_level.as_str());
        item.overdue = shift_age_days > overdue_threshold_days;
        item.overdue_days = if item.overdue {
            shift_age_days - overdue_threshold_days
        } else {
            0
        };
        item.ownership_violations = detect_ownership_violations(item);
    }

    let failed_continuity_runs = continuity_runs
        .iter()
        .filter(|item| item.status == "failed")
        .count() as i64;
    let restore_evidence_missing_runs = continuity_runs
        .iter()
        .filter(|item| item.evidence_count == 0)
        .count() as i64;

    let metrics = HandoverDigestMetrics {
        unresolved_incidents: incidents.len() as i64,
        escalation_backlog: ticket_backlog.len() as i64,
        failed_continuity_runs,
        pending_approvals: (workflow_approvals.len() + playbook_approvals.len()) as i64,
        restore_evidence_missing_runs,
        closed_items: items.iter().filter(|item| item.status == "closed").count() as i64,
        overdue_open_items: items
            .iter()
            .filter(|item| item.status == "open" && item.overdue)
            .count() as i64,
        ownership_gap_items: items
            .iter()
            .filter(|item| item.status == "open" && !item.ownership_violations.is_empty())
            .count() as i64,
    };

    let overdue_trend = load_handover_overdue_trend(&state.db, shift_date).await;

    let generated_at = items
        .iter()
        .map(|item| item.observed_at)
        .max()
        .unwrap_or_else(|| {
            Utc.from_utc_datetime(&shift_date.and_hms_opt(0, 0, 0).expect("midnight"))
        });

    Ok(HandoverDigestResponse {
        generated_at,
        digest_key: format!("handover-{}", shift_date.format("%Y-%m-%d")),
        shift_date: shift_date.format("%Y-%m-%d").to_string(),
        metrics,
        overdue_trend,
        items,
    })
}

fn resolve_handover_item_state(
    update: Option<&HandoverItemUpdateRecord>,
    default_next_owner: String,
    default_next_action: String,
) -> (String, String, String, Option<String>) {
    match update {
        Some(item) => (
            item.status.clone(),
            item.next_owner.clone(),
            item.next_action.clone(),
            item.note.clone(),
        ),
        None => (
            "open".to_string(),
            default_next_owner,
            default_next_action,
            None,
        ),
    }
}

fn build_handover_readiness_response(digest: &HandoverDigestResponse) -> HandoverReadinessResponse {
    let mut items = digest
        .items
        .iter()
        .map(build_handover_readiness_item)
        .collect::<Vec<_>>();
    sort_handover_readiness_items(&mut items);
    let summary = summarize_handover_readiness(items.as_slice());
    let readiness_state = derive_handover_readiness_state(digest, &summary);
    let reasons = derive_handover_readiness_reasons(digest, &summary);

    HandoverReadinessResponse {
        generated_at: Utc::now(),
        digest_key: digest.digest_key.clone(),
        shift_date: digest.shift_date.clone(),
        readiness_state: readiness_state.to_string(),
        summary,
        reasons,
        items,
    }
}

fn build_handover_readiness_item(item: &HandoverCarryoverItem) -> HandoverReadinessItem {
    let (readiness_state, reason) = derive_handover_item_readiness(item);
    HandoverReadinessItem {
        item_key: item.item_key.clone(),
        source_type: item.source_type.clone(),
        title: item.title.clone(),
        owner: item.owner.clone(),
        next_owner: item.next_owner.clone(),
        next_action: item.next_action.clone(),
        status: item.status.clone(),
        risk_level: item.risk_level.clone(),
        readiness_state: readiness_state.to_string(),
        priority_score: score_handover_readiness_item(item, readiness_state),
        reason,
        observed_at: item.observed_at,
        evidence_timestamp: item.observed_at,
    }
}

fn derive_handover_item_readiness(item: &HandoverCarryoverItem) -> (&'static str, String) {
    if item.status == "closed" {
        return ("ready", "Item is already closed for this shift.".to_string());
    }
    if item.overdue {
        return (
            "blocking",
            format!("Overdue by {} day(s).", item.overdue_days.max(1)),
        );
    }
    if !item.ownership_violations.is_empty() {
        return (
            "blocking",
            format!(
                "Ownership violation: {}.",
                item.ownership_violations.join(", ")
            ),
        );
    }
    if matches!(item.risk_level.as_str(), "critical" | "high") {
        return (
            "at_risk",
            format!("Open {} risk item requires explicit owner follow-up.", item.risk_level),
        );
    }
    ("at_risk", "Open carryover item requires handoff confirmation.".to_string())
}

fn score_handover_readiness_item(item: &HandoverCarryoverItem, readiness_state: &str) -> i32 {
    let risk_component = risk_rank(item.risk_level.as_str()) as i32 * 100;
    let state_component = match readiness_state {
        "blocking" => 90,
        "at_risk" => 40,
        _ => 0,
    };
    let overdue_component = (item.overdue_days.max(0) as i32) * 20;
    let ownership_component = item.ownership_violations.len() as i32 * 15;
    let open_component = if item.status == "open" { 10 } else { 0 };
    risk_component + state_component + overdue_component + ownership_component + open_component
}

fn sort_handover_readiness_items(items: &mut [HandoverReadinessItem]) {
    items.sort_by(|left, right| {
        right
            .priority_score
            .cmp(&left.priority_score)
            .then_with(|| right.observed_at.cmp(&left.observed_at))
            .then_with(|| left.item_key.cmp(&right.item_key))
    });
}

fn summarize_handover_readiness(items: &[HandoverReadinessItem]) -> HandoverReadinessSummary {
    let mut summary = HandoverReadinessSummary {
        total: items.len(),
        ..HandoverReadinessSummary::default()
    };
    for item in items {
        if item.status == "open" {
            summary.open_items += 1;
        } else {
            summary.closed_items += 1;
        }
        match item.readiness_state.as_str() {
            "blocking" => summary.blocking += 1,
            "at_risk" => summary.at_risk += 1,
            _ => summary.ready += 1,
        }
    }
    summary
}

fn derive_handover_readiness_state(
    digest: &HandoverDigestResponse,
    summary: &HandoverReadinessSummary,
) -> &'static str {
    if summary.blocking > 0
        || digest.metrics.overdue_open_items > 0
        || digest.metrics.ownership_gap_items > 0
    {
        "blocking"
    } else if summary.at_risk > 0 || summary.open_items > 0 {
        "at_risk"
    } else {
        "ready"
    }
}

fn derive_handover_readiness_reasons(
    digest: &HandoverDigestResponse,
    summary: &HandoverReadinessSummary,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if digest.metrics.overdue_open_items > 0 {
        reasons.push(format!(
            "overdue_open_items={}",
            digest.metrics.overdue_open_items
        ));
    }
    if digest.metrics.ownership_gap_items > 0 {
        reasons.push(format!(
            "ownership_gap_items={}",
            digest.metrics.ownership_gap_items
        ));
    }
    if summary.blocking > 0 {
        reasons.push(format!("blocking_items={}", summary.blocking));
    }
    if summary.at_risk > 0 {
        reasons.push(format!("at_risk_items={}", summary.at_risk));
    }
    if summary.open_items > 0 {
        reasons.push(format!("open_items={}", summary.open_items));
    }
    if reasons.is_empty() {
        reasons.push("no_open_blocker_or_risk_detected".to_string());
    }
    reasons
}

fn parse_week_start(value: Option<String>) -> AppResult<DateTime<Utc>> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return default_week_start();
            }
            let date = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
                AppError::Validation("week_start must use YYYY-MM-DD format".to_string())
            })?;
            Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight")))
        }
        None => default_week_start(),
    }
}

fn default_week_start() -> AppResult<DateTime<Utc>> {
    let today = Utc::now().date_naive();
    let weekday = today.weekday().number_from_monday() as i64;
    let monday = today - Duration::days(weekday - 1);
    Ok(Utc.from_utc_datetime(&monday.and_hms_opt(0, 0, 0).expect("midnight")))
}

fn parse_shift_date(value: Option<String>) -> AppResult<NaiveDate> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(Utc::now().date_naive());
            }
            NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
                AppError::Validation("shift_date must use YYYY-MM-DD format".to_string())
            })
        }
        None => Ok(Utc::now().date_naive()),
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
        } else if trimmed.len() > max_len {
            Some(trimmed[..max_len].to_string())
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn overdue_threshold_days_by_risk(risk_level: &str) -> i64 {
    match risk_level {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 2,
    }
}

fn detect_ownership_violations(item: &HandoverCarryoverItem) -> Vec<String> {
    let mut violations = Vec::new();
    let owner = item.owner.trim();
    let next_owner = item.next_owner.trim();
    let next_action = item.next_action.trim();

    if owner.is_empty() {
        violations.push("owner_missing".to_string());
    }
    if next_owner.is_empty() || next_owner.eq_ignore_ascii_case("ops-oncall") {
        violations.push("next_owner_unassigned".to_string());
    }
    if next_action.is_empty() {
        violations.push("next_action_missing".to_string());
    }

    violations
}

async fn load_handover_overdue_trend(
    db: &sqlx::PgPool,
    shift_date: NaiveDate,
) -> Vec<HandoverOverdueTrendPoint> {
    let trend_start = shift_date - Duration::days(6);
    let rows: Vec<HandoverTrendRow> = sqlx::query_as(
        "WITH latest AS (
            SELECT DISTINCT ON (shift_date, item_key)
                shift_date, item_key, status
            FROM ops_handover_item_updates
            WHERE shift_date >= $1
              AND shift_date <= $2
            ORDER BY shift_date, item_key, updated_at DESC, id DESC
         )
         SELECT shift_date,
                COUNT(*) FILTER (WHERE status = 'open') AS open_items
         FROM latest
         GROUP BY shift_date
         ORDER BY shift_date ASC",
    )
    .bind(trend_start)
    .bind(shift_date)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut points = Vec::new();
    let mut day = trend_start;
    while day <= shift_date {
        let open_items = rows
            .iter()
            .find(|item| item.shift_date == day)
            .map(|item| item.open_items)
            .unwrap_or(0);
        let shift_age_days = (Utc::now().date_naive() - day).num_days().max(0);
        let overdue_items = if shift_age_days > 1 { open_items } else { 0 };
        points.push(HandoverOverdueTrendPoint {
            shift_date: day.to_string(),
            open_items,
            overdue_items,
        });
        day += Duration::days(1);
    }
    points
}

fn risk_rank(value: &str) -> i32 {
    match value {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn handover_digest_to_csv(digest: &HandoverDigestResponse) -> String {
    let mut lines = vec![
        "section,field,value".to_string(),
        format!(
            "meta,digest_key,{}",
            escape_csv_cell(digest.digest_key.as_str())
        ),
        format!(
            "meta,generated_at,{}",
            escape_csv_cell(digest.generated_at.to_rfc3339().as_str())
        ),
        format!(
            "meta,shift_date,{}",
            escape_csv_cell(digest.shift_date.as_str())
        ),
        format!(
            "metrics,unresolved_incidents,{}",
            digest.metrics.unresolved_incidents
        ),
        format!(
            "metrics,escalation_backlog,{}",
            digest.metrics.escalation_backlog
        ),
        format!(
            "metrics,failed_continuity_runs,{}",
            digest.metrics.failed_continuity_runs
        ),
        format!(
            "metrics,pending_approvals,{}",
            digest.metrics.pending_approvals
        ),
        format!(
            "metrics,restore_evidence_missing_runs,{}",
            digest.metrics.restore_evidence_missing_runs
        ),
        format!("metrics,closed_items,{}", digest.metrics.closed_items),
        format!(
            "metrics,overdue_open_items,{}",
            digest.metrics.overdue_open_items
        ),
        format!(
            "metrics,ownership_gap_items,{}",
            digest.metrics.ownership_gap_items
        ),
        "trend,shift_date,open_items,overdue_items".to_string(),
    ];

    for point in &digest.overdue_trend {
        lines.push(format!(
            "trend,{},{},{}",
            escape_csv_cell(point.shift_date.as_str()),
            point.open_items,
            point.overdue_items
        ));
    }

    lines.push(
        "items,item_key,source_type,source_id,title,owner,next_owner,next_action,status,risk_level,overdue,overdue_days,ownership_violations,observed_at,source_ref,note".to_string(),
    );

    for item in &digest.items {
        lines.push(format!(
            "item,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            escape_csv_cell(item.item_key.as_str()),
            escape_csv_cell(item.source_type.as_str()),
            item.source_id,
            escape_csv_cell(item.title.as_str()),
            escape_csv_cell(item.owner.as_str()),
            escape_csv_cell(item.next_owner.as_str()),
            escape_csv_cell(item.next_action.as_str()),
            escape_csv_cell(item.status.as_str()),
            escape_csv_cell(item.risk_level.as_str()),
            item.overdue,
            item.overdue_days,
            escape_csv_cell(item.ownership_violations.join("|").as_str()),
            escape_csv_cell(item.observed_at.to_rfc3339().as_str()),
            escape_csv_cell(item.source_ref.as_str()),
            escape_csv_cell(item.note.as_deref().unwrap_or("")),
        ));
    }

    lines.join("\n")
}

fn handover_reminder_to_csv(reminder: &HandoverReminderResponse) -> String {
    let mut lines = vec![
        "section,field,value".to_string(),
        format!(
            "meta,digest_key,{}",
            escape_csv_cell(reminder.digest_key.as_str())
        ),
        format!(
            "meta,generated_at,{}",
            escape_csv_cell(reminder.generated_at.to_rfc3339().as_str())
        ),
        format!(
            "meta,shift_date,{}",
            escape_csv_cell(reminder.shift_date.as_str())
        ),
        format!("meta,total,{}", reminder.total),
        "items,item_key,source_type,source_id,risk_level,next_owner,next_action,overdue,overdue_days,ownership_violations".to_string(),
    ];

    for item in &reminder.items {
        lines.push(format!(
            "item,{},{},{},{},{},{},{},{},{}",
            escape_csv_cell(item.item_key.as_str()),
            escape_csv_cell(item.source_type.as_str()),
            item.source_id,
            escape_csv_cell(item.risk_level.as_str()),
            escape_csv_cell(item.next_owner.as_str()),
            escape_csv_cell(item.next_action.as_str()),
            item.overdue,
            item.overdue_days,
            escape_csv_cell(item.ownership_violations.join("|").as_str()),
        ));
    }

    lines.join("\n")
}

fn digest_to_csv(digest: &WeeklyDigestResponse) -> String {
    let mut lines = Vec::new();
    lines.push("field,value".to_string());
    lines.push(format!("digest_key,{}", digest.digest_key));
    lines.push(format!("generated_at,{}", digest.generated_at.to_rfc3339()));
    lines.push(format!("week_start,{}", digest.week_start));
    lines.push(format!("week_end,{}", digest.week_end));
    lines.push(format!(
        "open_critical_alerts,{}",
        digest.metrics.open_critical_alerts
    ));
    lines.push(format!(
        "open_warning_alerts,{}",
        digest.metrics.open_warning_alerts
    ));
    lines.push(format!(
        "suppressed_alert_threads,{}",
        digest.metrics.suppressed_alert_threads
    ));
    lines.push(format!(
        "stale_open_tickets,{}",
        digest.metrics.stale_open_tickets
    ));
    lines.push(format!(
        "workflow_approval_backlog,{}",
        digest.metrics.workflow_approval_backlog
    ));
    lines.push(format!(
        "playbook_approval_backlog,{}",
        digest.metrics.playbook_approval_backlog
    ));
    lines.push(format!(
        "backup_failed_policies,{}",
        digest.metrics.backup_failed_policies
    ));
    lines.push(format!(
        "drill_failed_policies,{}",
        digest.metrics.drill_failed_policies
    ));
    lines.push(format!(
        "continuity_runs_requiring_evidence,{}",
        digest.metrics.continuity_runs_requiring_evidence
    ));
    lines.push(format!(
        "continuity_runs_with_evidence,{}",
        digest.metrics.continuity_runs_with_evidence
    ));
    lines.push(format!(
        "continuity_runs_missing_evidence,{}",
        digest.metrics.continuity_runs_missing_evidence
    ));
    lines.push(format!(
        "locked_local_accounts,{}",
        digest.metrics.locked_local_accounts
    ));
    lines.push(format!(
        "local_accounts_without_mfa,{}",
        digest.metrics.local_accounts_without_mfa
    ));
    lines.push(format!("top_risks,{}", digest.top_risks.join(" | ")));
    lines.push(format!(
        "unresolved_items,{}",
        digest.unresolved_items.join(" | ")
    ));
    lines.push(format!(
        "recommended_actions,{}",
        digest.recommended_actions.join(" | ")
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Utc};

    use super::{
        HandoverCarryoverItem, HandoverDigestMetrics, HandoverDigestResponse,
        WeeklyDigestMetrics, WeeklyDigestResponse, build_handover_readiness_response,
        default_week_start, digest_to_csv, handover_digest_to_csv, parse_shift_date,
    };

    #[test]
    fn week_start_defaults_to_monday() {
        let monday = default_week_start().expect("week start");
        assert_eq!(monday.weekday().number_from_monday(), 1);
    }

    #[test]
    fn digest_csv_contains_core_fields() {
        let digest = WeeklyDigestResponse {
            generated_at: Utc::now(),
            digest_key: "weekly-2026-03-02".to_string(),
            week_start: "2026-03-02".to_string(),
            week_end: "2026-03-08".to_string(),
            metrics: WeeklyDigestMetrics {
                open_critical_alerts: 1,
                open_warning_alerts: 2,
                suppressed_alert_threads: 3,
                stale_open_tickets: 4,
                workflow_approval_backlog: 5,
                playbook_approval_backlog: 6,
                backup_failed_policies: 1,
                drill_failed_policies: 1,
                continuity_runs_requiring_evidence: 8,
                continuity_runs_with_evidence: 6,
                continuity_runs_missing_evidence: 2,
                locked_local_accounts: 0,
                local_accounts_without_mfa: 2,
            },
            top_risks: vec!["risk".to_string()],
            unresolved_items: vec!["item".to_string()],
            recommended_actions: vec!["action".to_string()],
        };

        let csv = digest_to_csv(&digest);
        assert!(csv.contains("digest_key"));
        assert!(csv.contains("open_critical_alerts,1"));
        assert!(csv.contains("continuity_runs_missing_evidence,2"));
        assert!(csv.contains("recommended_actions,action"));
    }

    #[test]
    fn parses_shift_date_with_expected_format() {
        let parsed = parse_shift_date(Some("2026-03-07".to_string())).expect("shift date");
        assert_eq!(parsed.to_string(), "2026-03-07");
        assert!(parse_shift_date(Some("2026/03/07".to_string())).is_err());
    }

    #[test]
    fn handover_csv_contains_digest_key_and_items() {
        let digest = HandoverDigestResponse {
            generated_at: Utc::now(),
            digest_key: "handover-2026-03-07".to_string(),
            shift_date: "2026-03-07".to_string(),
            metrics: HandoverDigestMetrics {
                unresolved_incidents: 1,
                escalation_backlog: 2,
                failed_continuity_runs: 1,
                pending_approvals: 3,
                restore_evidence_missing_runs: 1,
                closed_items: 0,
                overdue_open_items: 1,
                ownership_gap_items: 1,
            },
            overdue_trend: vec![],
            items: vec![HandoverCarryoverItem {
                item_key: "incident:10".to_string(),
                source_type: "incident_command".to_string(),
                source_id: 10,
                title: "incident".to_string(),
                owner: "ops-a".to_string(),
                next_owner: "ops-b".to_string(),
                next_action: "continue".to_string(),
                status: "open".to_string(),
                note: None,
                risk_level: "high".to_string(),
                observed_at: Utc::now(),
                source_ref: "/api/v1/ops/cockpit/incidents/10".to_string(),
                overdue: true,
                overdue_days: 2,
                ownership_violations: vec!["next_owner_unassigned".to_string()],
            }],
        };

        let csv = handover_digest_to_csv(&digest);
        assert!(csv.contains("handover-2026-03-07"));
        assert!(csv.contains("incident:10"));
        assert!(csv.contains("restore_evidence_missing_runs,1"));
    }

    #[test]
    fn handover_readiness_derivation_is_deterministic() {
        let now = Utc::now();
        let digest = HandoverDigestResponse {
            generated_at: now,
            digest_key: "handover-2026-03-18".to_string(),
            shift_date: "2026-03-18".to_string(),
            metrics: HandoverDigestMetrics {
                unresolved_incidents: 1,
                escalation_backlog: 1,
                failed_continuity_runs: 0,
                pending_approvals: 1,
                restore_evidence_missing_runs: 0,
                closed_items: 1,
                overdue_open_items: 1,
                ownership_gap_items: 0,
            },
            overdue_trend: vec![],
            items: vec![
                HandoverCarryoverItem {
                    item_key: "incident:20".to_string(),
                    source_type: "incident_command".to_string(),
                    source_id: 20,
                    title: "critical incident".to_string(),
                    owner: "ops-a".to_string(),
                    next_owner: "ops-a".to_string(),
                    next_action: "continue".to_string(),
                    status: "open".to_string(),
                    note: None,
                    risk_level: "critical".to_string(),
                    observed_at: now,
                    source_ref: "/api/v1/ops/cockpit/incidents/20".to_string(),
                    overdue: true,
                    overdue_days: 2,
                    ownership_violations: vec![],
                },
                HandoverCarryoverItem {
                    item_key: "ticket:100".to_string(),
                    source_type: "ticket_backlog".to_string(),
                    source_id: 100,
                    title: "ticket".to_string(),
                    owner: "ops-b".to_string(),
                    next_owner: "ops-b".to_string(),
                    next_action: "review".to_string(),
                    status: "closed".to_string(),
                    note: None,
                    risk_level: "medium".to_string(),
                    observed_at: now,
                    source_ref: "/api/v1/tickets/100".to_string(),
                    overdue: false,
                    overdue_days: 0,
                    ownership_violations: vec![],
                },
            ],
        };
        let readiness = build_handover_readiness_response(&digest);
        assert_eq!(readiness.readiness_state, "blocking");
        assert_eq!(readiness.summary.total, 2);
        assert_eq!(readiness.summary.blocking, 1);
        assert_eq!(readiness.summary.ready, 1);
        assert_eq!(readiness.items[0].item_key, "incident:20");
    }
}
