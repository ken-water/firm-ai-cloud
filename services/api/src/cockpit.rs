use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;
const STALE_PENDING_MINUTES: i64 = 15;
const STALE_STREAM_MINUTES: i64 = 20;
const MAX_DAILY_OPS_NOTE_LEN: usize = 1024;
const CHECKLIST_STATUS_PENDING: &str = "pending";
const CHECKLIST_STATUS_COMPLETED: &str = "completed";
const CHECKLIST_STATUS_SKIPPED: &str = "skipped";
const MAX_CHECKLIST_NOTE_LEN: usize = 1024;
const DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED: &str = "acknowledged";
const DAILY_OPS_FOLLOW_UP_STATE_COMPLETED: &str = "completed";
const DAILY_OPS_FOLLOW_UP_STATE_DEFERRED: &str = "deferred";
const GO_LIVE_NOTIFICATION_EVENT: &str = "runbook_risk.ticket_linked";
const GO_LIVE_DEFAULT_TICKET_POLICY_KEY: &str = "default-ticket-sla";
const GO_LIVE_FACTORY_ESCALATION_OWNER: &str = "ops-escalation";
const GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY: &str = "global";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/queue", get(get_daily_cockpit_queue))
        .route("/cockpit/daily-ops/briefing", get(get_daily_ops_briefing))
        .route(
            "/cockpit/daily-ops/follow-up-actions",
            post(apply_daily_ops_follow_up_action),
        )
        .route("/cockpit/next-actions", get(get_next_best_actions))
        .route("/cockpit/go-live/readiness", get(get_go_live_readiness))
        .route("/cockpit/checklists", get(get_ops_checklist))
        .route(
            "/cockpit/checklists/{template_key}/complete",
            post(complete_ops_checklist_item),
        )
        .route(
            "/cockpit/checklists/{template_key}/exception",
            post(mark_ops_checklist_exception),
        )
}

#[derive(Debug, Deserialize, Default)]
struct DailyCockpitQuery {
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct DailyOpsBriefingQuery {
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct DailyOpsFollowUpActionRequest {
    task_key: String,
    action: String,
    note: Option<String>,
    defer_until: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct NextBestActionQuery {
    site: Option<String>,
    department: Option<String>,
    shift_date: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct DailyCockpitQueueResponse {
    generated_at: DateTime<Utc>,
    scope: DailyCockpitScope,
    window: DailyCockpitWindow,
    items: Vec<DailyCockpitQueueItem>,
}

#[derive(Debug, Serialize)]
struct NextBestActionResponse {
    generated_at: DateTime<Utc>,
    scope: DailyCockpitScope,
    shift_date: String,
    total: usize,
    items: Vec<NextBestActionItem>,
}

#[derive(Debug, Serialize)]
struct DailyOpsBriefingResponse {
    generated_at: DateTime<Utc>,
    scope: DailyCockpitScope,
    summary: DailyOpsBriefingSummary,
    recommended_next_task_key: Option<String>,
    items: Vec<DailyOpsFollowUpItem>,
}

#[derive(Debug, Serialize, Default)]
struct DailyOpsBriefingSummary {
    total: usize,
    due_today: usize,
    overdue: usize,
    blocked: usize,
    completed: usize,
    deferred: usize,
    acknowledged: usize,
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
}

#[derive(Debug, Serialize, Clone)]
struct DailyOpsFollowUpItem {
    task_key: String,
    item_type: String,
    domain: String,
    status: String,
    follow_up_state: String,
    priority: String,
    owner: DailyOpsOwner,
    summary: String,
    reason: String,
    recommended_action: Option<DailyOpsRecommendedAction>,
    available_actions: Vec<DailyOpsTaskAction>,
    due_at: Option<DateTime<Utc>>,
    escalate_at: Option<DateTime<Utc>>,
    due_policy: DailyOpsDuePolicy,
    observed_at: DateTime<Utc>,
    acknowledged_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    deferred_until: Option<DateTime<Utc>>,
    evidence: Value,
}

#[derive(Debug, Serialize, Clone)]
struct DailyOpsRecommendedAction {
    action_key: String,
    label: String,
    description: String,
    action_type: String,
    href: Option<String>,
    api_path: Option<String>,
    method: Option<String>,
    body: Option<Value>,
    requires_write: bool,
}

#[derive(Debug, Serialize, Clone)]
struct DailyOpsTaskAction {
    action_key: String,
    label: String,
    requires_write: bool,
}

#[derive(Debug, Serialize, Clone)]
struct DailyOpsOwner {
    owner_ref: Option<String>,
    owner_state: String,
    source: String,
    reason: String,
}

#[derive(Debug, Serialize, Clone)]
struct DailyOpsDuePolicy {
    policy_key: String,
    due_window_minutes: i32,
    escalation_window_minutes: i32,
    source: String,
}

#[derive(Debug, Serialize)]
struct DailyOpsFollowUpActionResponse {
    task_key: String,
    action: String,
    actor: String,
    status_before: String,
    status_after: String,
    item_before: DailyOpsFollowUpItem,
    item_after: DailyOpsFollowUpItem,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct GoLiveReadinessDomainItem {
    domain_key: String,
    name: String,
    status: String,
    summary: String,
    reason: String,
    recommended_action: Option<GoLiveRemediationAction>,
    evidence: Value,
}

#[derive(Debug, Serialize, Clone)]
struct GoLiveRemediationAction {
    action_key: String,
    label: String,
    description: String,
    action_type: String,
    href: Option<String>,
    api_path: Option<String>,
    method: Option<String>,
    body: Option<Value>,
    requires_write: bool,
    auto_applicable: bool,
    blocked_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct GoLiveReadinessSummary {
    total: usize,
    ready: usize,
    warning: usize,
    blocking: usize,
}

#[derive(Debug, Serialize)]
struct GoLiveReadinessResponse {
    generated_at: DateTime<Utc>,
    overall_status: String,
    recommended_next_domain: Option<String>,
    summary: GoLiveReadinessSummary,
    domains: Vec<GoLiveReadinessDomainItem>,
}

#[derive(Debug, Serialize, Clone)]
struct NextBestActionItem {
    suggestion_key: String,
    domain: String,
    priority_score: i32,
    risk_level: String,
    reason: String,
    source_signal: String,
    observed_at: DateTime<Utc>,
    entity: Value,
    action: DailyCockpitAction,
}

#[derive(Debug, Serialize)]
struct DailyCockpitScope {
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize)]
struct DailyCockpitWindow {
    limit: u32,
    offset: u32,
    total: usize,
}

#[derive(Debug, Serialize, Clone)]
struct DailyCockpitQueueItem {
    queue_key: String,
    item_type: String,
    priority_score: i32,
    priority_level: String,
    rationale: String,
    rationale_details: Vec<String>,
    observed_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
    entity: Value,
    actions: Vec<DailyCockpitAction>,
}

#[derive(Debug, Serialize, Clone)]
struct DailyCockpitAction {
    key: String,
    label: String,
    href: Option<String>,
    api_path: Option<String>,
    method: Option<String>,
    body: Option<Value>,
    requires_write: bool,
}

#[derive(Debug, Clone)]
struct DailyOpsCandidateItem {
    task_key: String,
    item_type: String,
    domain: String,
    base_status: String,
    priority: String,
    owner: DailyOpsOwner,
    summary: String,
    reason: String,
    recommended_action: Option<DailyOpsRecommendedAction>,
    due_at: Option<DateTime<Utc>>,
    escalate_at: Option<DateTime<Utc>>,
    due_policy: DailyOpsDuePolicy,
    observed_at: DateTime<Utc>,
    evidence: Value,
}

#[derive(Debug, FromRow)]
struct DailyOpsFollowUpStateRow {
    task_key: String,
    item_type: String,
    follow_up_state: String,
    note: Option<String>,
    defer_until: Option<DateTime<Utc>>,
    actor: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DailyOpsRunbookRiskAggregateRow {
    template_key: String,
    template_name: String,
    executions: i64,
    failed: i64,
    latest_failed_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DailyOpsActivationFeedbackRow {
    step_key: String,
    template_key: Option<String>,
    feedback_kind: String,
    comment: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RunbookRiskOwnerRouteRow {
    template_key: String,
    owner_ref: String,
}

#[derive(Debug, FromRow)]
struct AlertQueueRow {
    id: i64,
    alert_source: String,
    alert_key: String,
    title: String,
    severity: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    asset_id: Option<i64>,
    last_seen_at: DateTime<Utc>,
    remediation_execution_id: Option<i64>,
    remediation_playbook_key: Option<String>,
    remediation_mode: Option<String>,
    remediation_status: Option<String>,
    remediation_created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct TicketQueueRow {
    id: i64,
    ticket_no: String,
    title: String,
    status: String,
    priority: String,
    assignee: Option<String>,
    updated_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, FromRow)]
struct SyncJobQueueRow {
    id: i64,
    asset_id: i64,
    asset_name: String,
    status: String,
    attempt: i32,
    max_attempts: i32,
    requested_at: DateTime<Utc>,
    run_after: DateTime<Utc>,
    last_error: Option<String>,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, FromRow)]
struct IncidentNextActionRow {
    alert_id: i64,
    title: String,
    severity: String,
    command_status: String,
    command_owner: String,
    updated_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, FromRow)]
struct TicketEscalationNextActionRow {
    id: i64,
    ticket_no: String,
    title: String,
    priority: String,
    status: String,
    assignee: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, FromRow)]
struct HandoverNextActionRow {
    item_key: String,
    source_type: String,
    source_id: i64,
    next_owner: String,
    next_action: String,
    note: Option<String>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct EscalationWindowRow {
    near_critical_minutes: i32,
    breach_critical_minutes: i32,
    near_high_minutes: i32,
    breach_high_minutes: i32,
    near_medium_minutes: i32,
    breach_medium_minutes: i32,
    near_low_minutes: i32,
    breach_low_minutes: i32,
    escalate_to_assignee: String,
}

#[derive(Debug, FromRow)]
struct MonitoringGoLiveSummaryRow {
    enabled_total: i64,
    reachable_total: i64,
    unreachable_total: i64,
    unknown_probe_total: i64,
}

#[derive(Debug, FromRow)]
struct NotificationGoLiveSummaryRow {
    enabled_channel_count: i64,
    enabled_subscription_count: i64,
}

#[derive(Debug, FromRow)]
struct TicketFollowupGoLiveRow {
    is_enabled: bool,
    escalate_to_assignee: String,
}

#[derive(Debug, FromRow)]
struct BackupGoLiveSummaryRow {
    total_policies: i64,
    stale_backup_policies: i64,
    drill_gap_policies: i64,
    closed_evidence_count: i64,
    open_evidence_count: i64,
}

#[derive(Debug, FromRow)]
struct RunbookExecutionGoLiveRow {
    mode: String,
    live_template_count: i32,
    preset_count: i64,
}

#[derive(Debug, FromRow)]
struct GoLiveOwnerSuggestionRow {
    owner_ref: String,
    notification_target: Option<String>,
}

#[derive(Debug, FromRow)]
struct GoLiveNotificationChannelSuggestionRow {
    name: String,
    channel_type: String,
    target: String,
}

#[derive(Debug, Deserialize, Default)]
struct OpsChecklistQuery {
    date: Option<String>,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct OpsChecklistUpdateRequest {
    date: Option<String>,
    site: Option<String>,
    department: Option<String>,
    note: Option<String>,
    mark_skipped: Option<bool>,
}

#[derive(Debug, Serialize)]
struct OpsChecklistResponse {
    generated_at: DateTime<Utc>,
    checklist_date: String,
    operator: String,
    scope: DailyCockpitScope,
    summary: OpsChecklistSummary,
    items: Vec<OpsChecklistItem>,
}

#[derive(Debug, Serialize)]
struct OpsChecklistSummary {
    total: usize,
    completed: usize,
    pending: usize,
    skipped: usize,
    overdue: usize,
}

#[derive(Debug, Serialize)]
struct OpsChecklistItem {
    template_key: String,
    title: String,
    description: Option<String>,
    frequency: String,
    due_weekday: Option<i16>,
    status: String,
    overdue: bool,
    exception_note: Option<String>,
    completed_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    guidance: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpsChecklistUpdateResponse {
    checklist_date: String,
    template_key: String,
    status: String,
    operator: String,
    scope: DailyCockpitScope,
    completed_at: Option<DateTime<Utc>>,
    exception_note: Option<String>,
}

#[derive(Debug, FromRow)]
struct OpsChecklistTemplateRow {
    id: i64,
    template_key: String,
    title: String,
    description: Option<String>,
    frequency: String,
    due_weekday: Option<i16>,
    guidance: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct OpsChecklistEntryRow {
    template_id: i64,
    status: String,
    exception_note: Option<String>,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

async fn get_daily_cockpit_queue(
    State(state): State<AppState>,
    Query(query): Query<DailyCockpitQuery>,
) -> AppResult<Json<DailyCockpitQueueResponse>> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let site = trim_optional(query.site, 128);
    let department = trim_optional(query.department, 128);

    let mut queue = Vec::new();
    queue.extend(build_alert_queue_items(
        fetch_alert_queue_rows(
            &state.db,
            site.as_deref(),
            department.as_deref(),
            MAX_LIMIT as i64,
        )
        .await?,
    ));
    queue.extend(build_ticket_queue_items(
        fetch_ticket_queue_rows(
            &state.db,
            site.as_deref(),
            department.as_deref(),
            MAX_LIMIT as i64,
        )
        .await?,
    ));
    queue.extend(build_sync_job_queue_items(
        fetch_sync_job_rows(
            &state.db,
            site.as_deref(),
            department.as_deref(),
            MAX_LIMIT as i64,
        )
        .await?,
    ));

    if let Some(stale_stream_item) =
        build_stale_stream_item(&state.db, site.as_deref(), department.as_deref()).await?
    {
        queue.push(stale_stream_item);
    }

    sort_daily_queue_items(&mut queue);

    let total = queue.len();
    let start = offset as usize;
    let end = start.saturating_add(limit as usize).min(total);
    let paged_items = if start >= total {
        Vec::new()
    } else {
        queue[start..end].to_vec()
    };

    Ok(Json(DailyCockpitQueueResponse {
        generated_at: Utc::now(),
        scope: DailyCockpitScope { site, department },
        window: DailyCockpitWindow {
            limit,
            offset,
            total,
        },
        items: paged_items,
    }))
}

async fn get_daily_ops_briefing(
    State(state): State<AppState>,
    Query(query): Query<DailyOpsBriefingQuery>,
) -> AppResult<Json<DailyOpsBriefingResponse>> {
    let limit = query.limit.unwrap_or(24).clamp(1, MAX_LIMIT);
    let site = trim_optional(query.site, 128);
    let department = trim_optional(query.department, 128);
    let now = Utc::now();

    let mut candidates = collect_daily_ops_candidates(
        &state,
        site.as_deref(),
        department.as_deref(),
        MAX_LIMIT as i64,
        now,
    )
    .await?;
    let state_by_task = load_daily_ops_follow_up_states(
        &state.db,
        &candidates
            .iter()
            .map(|item| item.task_key.clone())
            .collect::<Vec<_>>(),
    )
    .await?;

    let mut items = candidates
        .drain(..)
        .map(|item| {
            let task_key = item.task_key.clone();
            build_daily_ops_follow_up_item(item, state_by_task.get(task_key.as_str()), now)
        })
        .collect::<Vec<_>>();
    sort_daily_ops_items(&mut items);

    let summary = summarize_daily_ops_items(&items);
    let recommended_next_task_key = select_recommended_daily_ops_task(&items);
    if items.len() > limit as usize {
        items.truncate(limit as usize);
    }

    Ok(Json(DailyOpsBriefingResponse {
        generated_at: now,
        scope: DailyCockpitScope { site, department },
        summary,
        recommended_next_task_key,
        items,
    }))
}

async fn apply_daily_ops_follow_up_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DailyOpsFollowUpActionRequest>,
) -> AppResult<Json<DailyOpsFollowUpActionResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let task_key = trim_optional(Some(payload.task_key), 255)
        .ok_or_else(|| AppError::Validation("task_key is required".to_string()))?;
    let action = normalize_daily_ops_action(payload.action)?;
    let note = normalize_daily_ops_follow_up_note(payload.note)?;
    let defer_until = parse_daily_ops_defer_until(payload.defer_until, action.as_str())?;
    let now = Utc::now();
    if action == "defer" && defer_until.map(|value| value <= now).unwrap_or(true) {
        return Err(AppError::Validation(
            "defer_until must be a future RFC3339 timestamp".to_string(),
        ));
    }

    let mut candidates =
        collect_daily_ops_candidates(&state, None, None, MAX_LIMIT as i64, now).await?;
    let state_by_task = load_daily_ops_follow_up_states(
        &state.db,
        &candidates
            .iter()
            .map(|item| item.task_key.clone())
            .collect::<Vec<_>>(),
    )
    .await?;
    let candidate = candidates
        .drain(..)
        .find(|item| item.task_key == task_key)
        .ok_or_else(|| {
            AppError::Validation(format!("daily ops task '{task_key}' is not available"))
        })?;

    let item_before = build_daily_ops_follow_up_item(
        candidate.clone(),
        state_by_task.get(task_key.as_str()),
        now,
    );
    let state_after = upsert_daily_ops_follow_up_state(
        &state.db,
        &task_key,
        candidate.item_type.as_str(),
        action.as_str(),
        note.clone(),
        defer_until,
        actor.as_str(),
    )
    .await?;
    let item_after = build_daily_ops_follow_up_item(candidate, Some(&state_after), now);

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: format!("ops.daily_ops.follow_up.{action}"),
            target_type: "ops_daily_follow_up".to_string(),
            target_id: Some(task_key.clone()),
            result: "success".to_string(),
            message: Some(format!(
                "daily ops task '{}' updated via {}",
                task_key, action
            )),
            metadata: json!({
                "item_type": state_after.item_type,
                "note": note,
                "defer_until": defer_until,
                "status_before": item_before.status,
                "status_after": item_after.status,
                "follow_up_state_before": item_before.follow_up_state,
                "follow_up_state_after": item_after.follow_up_state,
            }),
        },
    )
    .await;

    Ok(Json(DailyOpsFollowUpActionResponse {
        task_key,
        action,
        actor,
        status_before: item_before.status.clone(),
        status_after: item_after.status.clone(),
        item_before,
        item_after,
        updated_at: state_after.updated_at,
    }))
}

async fn get_next_best_actions(
    State(state): State<AppState>,
    Query(query): Query<NextBestActionQuery>,
) -> AppResult<Json<NextBestActionResponse>> {
    let limit = query.limit.unwrap_or(24).clamp(1, MAX_LIMIT);
    let site_scope = normalize_scope_value(query.site);
    let department_scope = normalize_scope_value(query.department);
    let shift_date = parse_optional_date(query.shift_date)?;
    let now = Utc::now();

    let incident_rows = fetch_incident_next_action_rows(
        &state.db,
        if site_scope.is_empty() {
            None
        } else {
            Some(site_scope.as_str())
        },
        if department_scope.is_empty() {
            None
        } else {
            Some(department_scope.as_str())
        },
        MAX_LIMIT as i64,
    )
    .await?;

    let escalation_rows = fetch_escalation_next_action_rows(
        &state.db,
        if site_scope.is_empty() {
            None
        } else {
            Some(site_scope.as_str())
        },
        if department_scope.is_empty() {
            None
        } else {
            Some(department_scope.as_str())
        },
        MAX_LIMIT as i64,
    )
    .await?;
    let escalation_window = load_default_escalation_window(&state.db).await?;

    let handover_rows =
        fetch_handover_next_action_rows(&state.db, shift_date, MAX_LIMIT as i64).await?;

    let mut items = Vec::new();
    items.extend(build_incident_next_actions(incident_rows));
    items.extend(build_escalation_next_actions(
        escalation_rows,
        &escalation_window,
        now,
    ));
    items.extend(build_handover_next_actions(handover_rows, shift_date));
    sort_next_best_actions(&mut items);
    if items.len() > limit as usize {
        items.truncate(limit as usize);
    }

    Ok(Json(NextBestActionResponse {
        generated_at: now,
        scope: DailyCockpitScope {
            site: if site_scope.is_empty() {
                None
            } else {
                Some(site_scope)
            },
            department: if department_scope.is_empty() {
                None
            } else {
                Some(department_scope)
            },
        },
        shift_date: shift_date.to_string(),
        total: items.len(),
        items,
    }))
}

async fn get_go_live_readiness(
    State(state): State<AppState>,
) -> AppResult<Json<GoLiveReadinessResponse>> {
    let domains = collect_go_live_readiness_domains(&state).await?;
    let summary = summarize_go_live_readiness(&domains);
    let overall_status = derive_go_live_overall_status(&summary);
    let recommended_next_domain = select_next_go_live_domain(&domains);

    Ok(Json(GoLiveReadinessResponse {
        generated_at: Utc::now(),
        overall_status,
        recommended_next_domain,
        summary,
        domains,
    }))
}

async fn get_ops_checklist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OpsChecklistQuery>,
) -> AppResult<Json<OpsChecklistResponse>> {
    let operator = resolve_auth_user(&state, &headers).await?;
    let checklist_date = parse_optional_date(query.date)?;
    let site_scope = normalize_scope_value(query.site);
    let department_scope = normalize_scope_value(query.department);

    let templates = load_ops_checklist_templates(&state.db).await?;
    let entries = load_ops_checklist_entries(
        &state.db,
        checklist_date,
        operator.as_str(),
        site_scope.as_str(),
        department_scope.as_str(),
    )
    .await?;
    let response = build_ops_checklist_response(
        checklist_date,
        operator,
        site_scope,
        department_scope,
        templates,
        entries,
    );

    Ok(Json(response))
}

async fn complete_ops_checklist_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(template_key): Path<String>,
    Json(payload): Json<OpsChecklistUpdateRequest>,
) -> AppResult<Json<OpsChecklistUpdateResponse>> {
    let operator = resolve_auth_user(&state, &headers).await?;
    let checklist_date = parse_optional_date(payload.date)?;
    let site_scope = normalize_scope_value(payload.site);
    let department_scope = normalize_scope_value(payload.department);
    let note = normalize_optional_note(payload.note)?;

    let (template, entry) = upsert_ops_checklist_status(
        &state.db,
        template_key.as_str(),
        checklist_date,
        operator.as_str(),
        site_scope.as_str(),
        department_scope.as_str(),
        CHECKLIST_STATUS_COMPLETED,
        note.clone(),
    )
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: operator.clone(),
            action: "ops.checklist.complete".to_string(),
            target_type: "ops_checklist".to_string(),
            target_id: Some(format!("{}:{}", template.template_key, checklist_date)),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.template_key,
                "checklist_date": checklist_date.to_string(),
                "site": site_scope,
                "department": department_scope,
                "status": entry.status,
            }),
        },
    )
    .await;

    Ok(Json(OpsChecklistUpdateResponse {
        checklist_date: checklist_date.to_string(),
        template_key: template.template_key,
        status: entry.status,
        operator,
        scope: DailyCockpitScope {
            site: if site_scope.is_empty() {
                None
            } else {
                Some(site_scope)
            },
            department: if department_scope.is_empty() {
                None
            } else {
                Some(department_scope)
            },
        },
        completed_at: entry.completed_at,
        exception_note: entry.exception_note,
    }))
}

async fn mark_ops_checklist_exception(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(template_key): Path<String>,
    Json(payload): Json<OpsChecklistUpdateRequest>,
) -> AppResult<Json<OpsChecklistUpdateResponse>> {
    let operator = resolve_auth_user(&state, &headers).await?;
    let checklist_date = parse_optional_date(payload.date)?;
    let site_scope = normalize_scope_value(payload.site);
    let department_scope = normalize_scope_value(payload.department);
    let note = normalize_optional_note(payload.note)?.ok_or_else(|| {
        AppError::Validation("note is required when recording checklist exception".to_string())
    })?;
    let status = if payload.mark_skipped.unwrap_or(true) {
        CHECKLIST_STATUS_SKIPPED
    } else {
        CHECKLIST_STATUS_PENDING
    };

    let (template, entry) = upsert_ops_checklist_status(
        &state.db,
        template_key.as_str(),
        checklist_date,
        operator.as_str(),
        site_scope.as_str(),
        department_scope.as_str(),
        status,
        Some(note.clone()),
    )
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: operator.clone(),
            action: "ops.checklist.exception".to_string(),
            target_type: "ops_checklist".to_string(),
            target_id: Some(format!("{}:{}", template.template_key, checklist_date)),
            result: "success".to_string(),
            message: Some(note),
            metadata: json!({
                "template_key": template.template_key,
                "checklist_date": checklist_date.to_string(),
                "site": site_scope,
                "department": department_scope,
                "status": entry.status,
            }),
        },
    )
    .await;

    Ok(Json(OpsChecklistUpdateResponse {
        checklist_date: checklist_date.to_string(),
        template_key: template.template_key,
        status: entry.status,
        operator,
        scope: DailyCockpitScope {
            site: if site_scope.is_empty() {
                None
            } else {
                Some(site_scope)
            },
            department: if department_scope.is_empty() {
                None
            } else {
                Some(department_scope)
            },
        },
        completed_at: entry.completed_at,
        exception_note: entry.exception_note,
    }))
}

async fn load_ops_checklist_templates(
    db: &sqlx::PgPool,
) -> AppResult<Vec<OpsChecklistTemplateRow>> {
    let rows: Vec<OpsChecklistTemplateRow> = sqlx::query_as(
        "SELECT id, template_key, title, description, frequency, due_weekday, guidance
         FROM ops_checklist_templates
         WHERE is_enabled = TRUE
         ORDER BY sort_order ASC, template_key ASC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

async fn load_ops_checklist_entries(
    db: &sqlx::PgPool,
    checklist_date: NaiveDate,
    operator: &str,
    site: &str,
    department: &str,
) -> AppResult<Vec<OpsChecklistEntryRow>> {
    let rows: Vec<OpsChecklistEntryRow> = sqlx::query_as(
        "SELECT template_id, status, exception_note, completed_at, updated_at
         FROM ops_checklist_entries
         WHERE check_date = $1
           AND operator = $2
           AND site = $3
           AND department = $4",
    )
    .bind(checklist_date)
    .bind(operator)
    .bind(site)
    .bind(department)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

fn build_ops_checklist_response(
    checklist_date: NaiveDate,
    operator: String,
    site_scope: String,
    department_scope: String,
    templates: Vec<OpsChecklistTemplateRow>,
    entries: Vec<OpsChecklistEntryRow>,
) -> OpsChecklistResponse {
    let today = Utc::now().date_naive();
    let weekday = checklist_date.weekday().number_from_monday() as i16;
    let entry_map = entries
        .into_iter()
        .map(|item| (item.template_id, item))
        .collect::<HashMap<_, _>>();

    let mut items = Vec::new();
    let mut summary = OpsChecklistSummary {
        total: 0,
        completed: 0,
        pending: 0,
        skipped: 0,
        overdue: 0,
    };

    for template in templates {
        if template.frequency == "weekly" {
            let due_weekday = template.due_weekday.unwrap_or(1);
            if due_weekday != weekday {
                continue;
            }
        }

        let entry = entry_map.get(&template.id);
        let status = entry
            .map(|item| item.status.as_str())
            .unwrap_or(CHECKLIST_STATUS_PENDING);
        let overdue = status == CHECKLIST_STATUS_PENDING && checklist_date < today;

        summary.total += 1;
        if status == CHECKLIST_STATUS_COMPLETED {
            summary.completed += 1;
        } else if status == CHECKLIST_STATUS_SKIPPED {
            summary.skipped += 1;
        } else {
            summary.pending += 1;
        }
        if overdue {
            summary.overdue += 1;
        }

        items.push(OpsChecklistItem {
            template_key: template.template_key,
            title: template.title,
            description: template.description,
            frequency: template.frequency,
            due_weekday: template.due_weekday,
            status: status.to_string(),
            overdue,
            exception_note: entry.and_then(|item| item.exception_note.clone()),
            completed_at: entry.and_then(|item| item.completed_at),
            updated_at: entry.map(|item| item.updated_at),
            guidance: template.guidance,
        });
    }

    OpsChecklistResponse {
        generated_at: Utc::now(),
        checklist_date: checklist_date.to_string(),
        operator,
        scope: DailyCockpitScope {
            site: if site_scope.is_empty() {
                None
            } else {
                Some(site_scope)
            },
            department: if department_scope.is_empty() {
                None
            } else {
                Some(department_scope)
            },
        },
        summary,
        items,
    }
}

async fn upsert_ops_checklist_status(
    db: &sqlx::PgPool,
    template_key: &str,
    checklist_date: NaiveDate,
    operator: &str,
    site: &str,
    department: &str,
    status: &str,
    note: Option<String>,
) -> AppResult<(OpsChecklistTemplateRow, OpsChecklistEntryRow)> {
    let template = resolve_ops_checklist_template(db, template_key).await?;
    if template.frequency == "weekly" {
        let expected_weekday = template.due_weekday.unwrap_or(1);
        let actual_weekday = checklist_date.weekday().number_from_monday() as i16;
        if expected_weekday != actual_weekday {
            return Err(AppError::Validation(format!(
                "weekly checklist '{}' is due on weekday {}",
                template.template_key, expected_weekday
            )));
        }
    }

    let entry: OpsChecklistEntryRow = sqlx::query_as(
        "INSERT INTO ops_checklist_entries
            (template_id, check_date, operator, site, department, status, exception_note, completed_at, updated_at)
         VALUES
            ($1, $2, $3, $4, $5, $6, $7, CASE WHEN $6 = 'completed' THEN NOW() ELSE NULL END, NOW())
         ON CONFLICT (template_id, check_date, operator, site, department)
         DO UPDATE SET
            status = EXCLUDED.status,
            exception_note = EXCLUDED.exception_note,
            completed_at = CASE WHEN EXCLUDED.status = 'completed' THEN NOW() ELSE NULL END,
            updated_at = NOW()
         RETURNING template_id, status, exception_note, completed_at, updated_at",
    )
    .bind(template.id)
    .bind(checklist_date)
    .bind(operator)
    .bind(site)
    .bind(department)
    .bind(status)
    .bind(note)
    .fetch_one(db)
    .await?;

    Ok((template, entry))
}

async fn resolve_ops_checklist_template(
    db: &sqlx::PgPool,
    template_key: &str,
) -> AppResult<OpsChecklistTemplateRow> {
    let normalized = template_key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("template_key is required".to_string()));
    }

    let template: Option<OpsChecklistTemplateRow> = sqlx::query_as(
        "SELECT id, template_key, title, description, frequency, due_weekday, guidance
         FROM ops_checklist_templates
         WHERE template_key = $1
           AND is_enabled = TRUE
         LIMIT 1",
    )
    .bind(normalized)
    .fetch_optional(db)
    .await?;

    template.ok_or_else(|| AppError::NotFound("checklist template not found".to_string()))
}

fn parse_optional_date(raw: Option<String>) -> AppResult<NaiveDate> {
    match raw {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(Utc::now().date_naive())
            } else {
                NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
                    .map_err(|_| AppError::Validation("date must be YYYY-MM-DD format".to_string()))
            }
        }
        None => Ok(Utc::now().date_naive()),
    }
}

fn normalize_scope_value(raw: Option<String>) -> String {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn normalize_optional_note(raw: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_CHECKLIST_NOTE_LEN {
        return Err(AppError::Validation(format!(
            "note length must be <= {MAX_CHECKLIST_NOTE_LEN}"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

async fn fetch_alert_queue_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AlertQueueRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            a.id,
            a.alert_source,
            a.alert_key,
            a.title,
            a.severity,
            a.status,
            a.site,
            a.department,
            a.asset_id,
            a.last_seen_at,
            remediation.id AS remediation_execution_id,
            remediation.playbook_key AS remediation_playbook_key,
            remediation.mode AS remediation_mode,
            remediation.status AS remediation_status,
            remediation.created_at AS remediation_created_at
         FROM unified_alerts a
         LEFT JOIN LATERAL (
            SELECT e.id, e.playbook_key, e.mode, e.status, e.created_at
            FROM workflow_playbook_executions e
            WHERE e.related_alert_id = a.id
            ORDER BY e.created_at DESC, e.id DESC
            LIMIT 1
         ) AS remediation ON TRUE
         WHERE a.status IN ('open', 'acknowledged')",
    );

    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }

    builder
        .push(" ORDER BY a.last_seen_at DESC, a.id DESC LIMIT ")
        .push_bind(limit);

    let rows: Vec<AlertQueueRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows)
}

async fn fetch_ticket_queue_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<TicketQueueRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            t.id,
            t.ticket_no,
            t.title,
            t.status,
            t.priority,
            t.assignee,
            t.updated_at,
            scope_asset.site,
            scope_asset.department
         FROM tickets t
         LEFT JOIN LATERAL (
            SELECT a.site, a.department
            FROM ticket_asset_links l
            INNER JOIN assets a ON a.id = l.asset_id
            WHERE l.ticket_id = t.id
            ORDER BY a.id ASC
            LIMIT 1
         ) AS scope_asset ON TRUE
         WHERE t.status IN ('open', 'in_progress')",
    );

    if let Some(site) = site {
        builder.push(
            " AND EXISTS (
                SELECT 1
                FROM ticket_asset_links l2
                INNER JOIN assets a2 ON a2.id = l2.asset_id
                WHERE l2.ticket_id = t.id
                  AND a2.site = ",
        );
        builder.push_bind(site).push(")");
    }

    if let Some(department) = department {
        builder.push(
            " AND EXISTS (
                SELECT 1
                FROM ticket_asset_links l3
                INNER JOIN assets a3 ON a3.id = l3.asset_id
                WHERE l3.ticket_id = t.id
                  AND a3.department = ",
        );
        builder.push_bind(department).push(")");
    }

    builder
        .push(" ORDER BY t.updated_at DESC, t.id DESC LIMIT ")
        .push_bind(limit);

    let rows: Vec<TicketQueueRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows)
}

async fn fetch_sync_job_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<SyncJobQueueRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            j.id,
            j.asset_id,
            a.name AS asset_name,
            j.status,
            j.attempt,
            j.max_attempts,
            j.requested_at,
            j.run_after,
            j.last_error,
            a.site,
            a.department
         FROM cmdb_monitoring_sync_jobs j
         INNER JOIN assets a ON a.id = j.asset_id
         WHERE (
            j.status IN ('failed', 'dead_letter')
            OR (j.status = 'pending' AND j.run_after <= NOW() - (",
    );
    builder
        .push_bind(STALE_PENDING_MINUTES as i32)
        .push(" * INTERVAL '1 minute')))");

    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }

    builder
        .push(" ORDER BY j.requested_at DESC, j.id DESC LIMIT ")
        .push_bind(limit);

    let rows: Vec<SyncJobQueueRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows)
}

async fn fetch_incident_next_action_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<IncidentNextActionRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            c.alert_id,
            COALESCE(a.title, concat('alert #', c.alert_id::text)) AS title,
            COALESCE(a.severity, 'warning') AS severity,
            c.command_status,
            c.command_owner,
            c.updated_at,
            a.site,
            a.department
         FROM ops_incident_commands c
         LEFT JOIN unified_alerts a ON a.id = c.alert_id
         WHERE c.command_status IN ('triage', 'in_progress', 'blocked')",
    );

    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }
    builder
        .push(" ORDER BY c.updated_at DESC, c.alert_id DESC LIMIT ")
        .push_bind(limit);

    let rows: Vec<IncidentNextActionRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows)
}

async fn fetch_escalation_next_action_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<TicketEscalationNextActionRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            t.id,
            t.ticket_no,
            t.title,
            t.priority,
            t.status,
            t.assignee,
            t.created_at,
            t.updated_at,
            scope_asset.site,
            scope_asset.department
         FROM tickets t
         LEFT JOIN LATERAL (
            SELECT a.site, a.department
            FROM ticket_asset_links l
            INNER JOIN assets a ON a.id = l.asset_id
            WHERE l.ticket_id = t.id
            ORDER BY a.id ASC
            LIMIT 1
         ) AS scope_asset ON TRUE
         WHERE t.status IN ('open', 'in_progress')",
    );

    if let Some(site) = site {
        builder.push(
            " AND EXISTS (
                SELECT 1
                FROM ticket_asset_links l2
                INNER JOIN assets a2 ON a2.id = l2.asset_id
                WHERE l2.ticket_id = t.id
                  AND a2.site = ",
        );
        builder.push_bind(site).push(")");
    }
    if let Some(department) = department {
        builder.push(
            " AND EXISTS (
                SELECT 1
                FROM ticket_asset_links l3
                INNER JOIN assets a3 ON a3.id = l3.asset_id
                WHERE l3.ticket_id = t.id
                  AND a3.department = ",
        );
        builder.push_bind(department).push(")");
    }
    builder
        .push(" ORDER BY t.created_at ASC, t.id ASC LIMIT ")
        .push_bind(limit);

    let rows: Vec<TicketEscalationNextActionRow> = builder.build_query_as().fetch_all(db).await?;
    Ok(rows)
}

async fn fetch_handover_next_action_rows(
    db: &sqlx::PgPool,
    shift_date: NaiveDate,
    limit: i64,
) -> AppResult<Vec<HandoverNextActionRow>> {
    let rows: Vec<HandoverNextActionRow> = sqlx::query_as(
        "SELECT item_key, source_type, source_id, next_owner, next_action, note, updated_at
         FROM ops_handover_item_updates
         WHERE shift_date = $1
           AND status = 'open'
         ORDER BY updated_at DESC, id DESC
         LIMIT $2",
    )
    .bind(shift_date)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

async fn load_default_escalation_window(db: &sqlx::PgPool) -> AppResult<EscalationWindowRow> {
    let row: Option<EscalationWindowRow> = sqlx::query_as(
        "SELECT
            near_critical_minutes,
            breach_critical_minutes,
            near_high_minutes,
            breach_high_minutes,
            near_medium_minutes,
            breach_medium_minutes,
            near_low_minutes,
            breach_low_minutes,
            escalate_to_assignee
         FROM ticket_escalation_policies
         WHERE policy_key = 'default-ticket-sla'
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;

    Ok(row.unwrap_or(EscalationWindowRow {
        near_critical_minutes: 30,
        breach_critical_minutes: 60,
        near_high_minutes: 60,
        breach_high_minutes: 120,
        near_medium_minutes: 120,
        breach_medium_minutes: 240,
        near_low_minutes: 240,
        breach_low_minutes: 480,
        escalate_to_assignee: "ops-escalation".to_string(),
    }))
}

fn build_incident_next_actions(rows: Vec<IncidentNextActionRow>) -> Vec<NextBestActionItem> {
    rows.into_iter()
        .map(|row| {
            let severity_score = match row.severity.as_str() {
                "critical" => 940,
                "warning" => 860,
                "info" => 800,
                _ => 760,
            };
            let status_boost = match row.command_status.as_str() {
                "blocked" => 40,
                "in_progress" => 25,
                "triage" => 15,
                _ => 0,
            };
            let priority_score = (severity_score + status_boost).min(999);
            let risk_level = if row.command_status == "blocked" || row.severity == "critical" {
                "critical"
            } else {
                "high"
            };

            NextBestActionItem {
                suggestion_key: format!("incident:{}", row.alert_id),
                domain: "incident".to_string(),
                priority_score,
                risk_level: risk_level.to_string(),
                reason: format!(
                    "Incident command status '{}' for alert #{} is unresolved.",
                    row.command_status, row.alert_id
                ),
                source_signal: format!(
                    "ops_incident_commands.command_status={} + unified_alerts.severity={}",
                    row.command_status, row.severity
                ),
                observed_at: row.updated_at,
                entity: json!({
                    "alert_id": row.alert_id,
                    "title": row.title,
                    "severity": row.severity,
                    "command_status": row.command_status,
                    "owner": row.command_owner,
                    "site": row.site,
                    "department": row.department,
                }),
                action: DailyCockpitAction {
                    key: "incident-command-follow-up".to_string(),
                    label: "Update Incident Command".to_string(),
                    href: Some("#/overview".to_string()),
                    api_path: Some(format!(
                        "/api/v1/ops/cockpit/incidents/{}/command",
                        row.alert_id
                    )),
                    method: Some("POST".to_string()),
                    body: Some(json!({
                        "status": "in_progress",
                        "owner": row.command_owner,
                        "summary": "updated from next-best-action assistant"
                    })),
                    requires_write: true,
                },
            }
        })
        .collect()
}

fn build_escalation_next_actions(
    rows: Vec<TicketEscalationNextActionRow>,
    window: &EscalationWindowRow,
    now: DateTime<Utc>,
) -> Vec<NextBestActionItem> {
    rows.into_iter()
        .filter_map(|row| {
            let age_minutes = (now - row.created_at).num_minutes().max(0);
            let (near_minutes, breach_minutes) =
                escalation_threshold_for_priority(window, row.priority.as_str());
            if age_minutes < near_minutes as i64 {
                return None;
            }

            let base_score = match row.priority.as_str() {
                "critical" => 930,
                "high" => 860,
                "medium" => 780,
                "low" => 720,
                _ => 700,
            };
            let breach_boost = if age_minutes >= breach_minutes as i64 {
                60
            } else {
                20
            };
            let priority_score = (base_score + breach_boost + age_minutes.min(90) as i32).min(999);
            let risk_level = if age_minutes >= breach_minutes as i64 {
                "critical"
            } else if matches!(row.priority.as_str(), "critical" | "high") {
                "high"
            } else {
                "medium"
            };

            Some(NextBestActionItem {
                suggestion_key: format!("escalation:{}", row.id),
                domain: "escalation".to_string(),
                priority_score,
                risk_level: risk_level.to_string(),
                reason: format!(
                    "Ticket {} has aged {} minutes (near={}, breach={}).",
                    row.ticket_no, age_minutes, near_minutes, breach_minutes
                ),
                source_signal: format!(
                    "tickets.priority={} + ticket_escalation_policies.default-ticket-sla owner={}",
                    row.priority, window.escalate_to_assignee
                ),
                observed_at: row.updated_at,
                entity: json!({
                    "ticket_id": row.id,
                    "ticket_no": row.ticket_no,
                    "title": row.title,
                    "priority": row.priority,
                    "status": row.status,
                    "assignee": row.assignee,
                    "age_minutes": age_minutes,
                    "near_minutes": near_minutes,
                    "breach_minutes": breach_minutes,
                    "site": row.site,
                    "department": row.department,
                }),
                action: DailyCockpitAction {
                    key: "run-ticket-escalation-dry-run".to_string(),
                    label: "Run Escalation Dry-Run".to_string(),
                    href: Some("#/tickets".to_string()),
                    api_path: Some("/api/v1/tickets/escalation/run".to_string()),
                    method: Some("POST".to_string()),
                    body: Some(json!({
                        "dry_run": true,
                        "note": format!("next-best-action for {}", row.ticket_no)
                    })),
                    requires_write: true,
                },
            })
        })
        .collect()
}

fn build_handover_next_actions(
    rows: Vec<HandoverNextActionRow>,
    shift_date: NaiveDate,
) -> Vec<NextBestActionItem> {
    rows.into_iter()
        .map(|row| {
            let (priority_score, risk_level) = match row.source_type.as_str() {
                "incident_command" => (840, "high"),
                "continuity_run" => (860, "critical"),
                "playbook_approval" => (800, "high"),
                "ticket_backlog" => (780, "medium"),
                _ => (760, "medium"),
            };

            NextBestActionItem {
                suggestion_key: format!("handover:{}", row.item_key),
                domain: "handover".to_string(),
                priority_score,
                risk_level: risk_level.to_string(),
                reason: format!(
                    "Carryover item '{}' remains open for shift {}.",
                    row.item_key, shift_date
                ),
                source_signal: format!(
                    "ops_handover_item_updates.status=open + source_type={}",
                    row.source_type
                ),
                observed_at: row.updated_at,
                entity: json!({
                    "item_key": row.item_key,
                    "source_type": row.source_type,
                    "source_id": row.source_id,
                    "next_owner": row.next_owner,
                    "next_action": row.next_action,
                    "note": row.note,
                    "shift_date": shift_date.to_string(),
                }),
                action: DailyCockpitAction {
                    key: "close-handover-item".to_string(),
                    label: "Close Handover Item".to_string(),
                    href: Some("#/overview".to_string()),
                    api_path: Some(format!(
                        "/api/v1/ops/cockpit/handover-digest/items/{}/close",
                        row.item_key
                    )),
                    method: Some("POST".to_string()),
                    body: Some(json!({
                        "shift_date": shift_date.to_string(),
                        "source_type": row.source_type,
                        "source_id": row.source_id,
                        "next_owner": row.next_owner,
                        "next_action": row.next_action,
                        "note": row.note,
                    })),
                    requires_write: true,
                },
            }
        })
        .collect()
}

fn escalation_threshold_for_priority(window: &EscalationWindowRow, priority: &str) -> (i32, i32) {
    match priority.trim().to_ascii_lowercase().as_str() {
        "critical" => (window.near_critical_minutes, window.breach_critical_minutes),
        "high" => (window.near_high_minutes, window.breach_high_minutes),
        "medium" => (window.near_medium_minutes, window.breach_medium_minutes),
        _ => (window.near_low_minutes, window.breach_low_minutes),
    }
}

async fn collect_go_live_readiness_domains(
    state: &AppState,
) -> AppResult<Vec<GoLiveReadinessDomainItem>> {
    Ok(vec![
        build_authentication_go_live_domain(state).await?,
        build_monitoring_go_live_domain(&state.db).await?,
        build_operator_notifications_go_live_domain(&state.db).await?,
        build_ticket_followup_go_live_domain(&state.db).await?,
        build_backup_restore_go_live_domain(&state.db).await?,
        build_runbook_execution_go_live_domain(&state.db).await?,
    ])
}

async fn build_authentication_go_live_domain(
    state: &AppState,
) -> AppResult<GoLiveReadinessDomainItem> {
    let oidc_ready = state.oidc.enabled
        && state.oidc.redirect_uri.is_some()
        && (state.oidc.dev_mode_enabled
            || (state.oidc.authorization_endpoint.is_some()
                && state.oidc.token_endpoint.is_some()
                && state.oidc.userinfo_endpoint.is_some()
                && state.oidc.client_id.is_some()
                && state.oidc.client_secret.is_some()));
    let local_fallback_mode = state.local_auth.fallback_mode.as_str();
    let break_glass_count = state.local_auth.break_glass_users.len();
    let local_ready = match local_fallback_mode {
        "allow_all" => true,
        "break_glass_only" => break_glass_count > 0,
        "disabled" => false,
        _ => false,
    };

    let (status, summary, reason) = if !state.rbac_enabled {
        (
            "blocking",
            "RBAC guard is disabled.".to_string(),
            "Protected routes run without permission checks, which is unsafe for production go-live."
                .to_string(),
        )
    } else if state.oidc.enabled && !oidc_ready {
        (
            "blocking",
            "OIDC is enabled but incomplete.".to_string(),
            "Enterprise SSO is partially configured and may fail during operator login."
                .to_string(),
        )
    } else if oidc_ready {
        (
            "ready",
            "Authentication controls are production-capable.".to_string(),
            "RBAC is enabled and OIDC configuration is complete for the current mode.".to_string(),
        )
    } else if local_ready {
        (
            "warning",
            "Platform relies on local/header fallback authentication.".to_string(),
            format!(
                "RBAC is enabled, but enterprise SSO is not complete; fallback mode is '{}'.",
                local_fallback_mode
            ),
        )
    } else {
        (
            "blocking",
            "No safe operator login path is configured.".to_string(),
            "OIDC is not ready and local fallback is disabled or missing break-glass users."
                .to_string(),
        )
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "authentication".to_string(),
        name: "Authentication readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action: Some(link_go_live_action(
            "go_live.authentication.review",
            "Review authentication setup",
            "Review RBAC, OIDC, and local fallback settings before production rollout.",
            "#/overview",
        )),
        evidence: json!({
            "rbac_enabled": state.rbac_enabled,
            "oidc_enabled": state.oidc.enabled,
            "oidc_ready": oidc_ready,
            "oidc_dev_mode_enabled": state.oidc.dev_mode_enabled,
            "local_fallback_mode": local_fallback_mode,
            "break_glass_user_count": break_glass_count,
        }),
    })
}

async fn build_monitoring_go_live_domain(
    db: &sqlx::PgPool,
) -> AppResult<GoLiveReadinessDomainItem> {
    let row: MonitoringGoLiveSummaryRow = sqlx::query_as(
        "SELECT
            COUNT(*) FILTER (WHERE is_enabled = TRUE) AS enabled_total,
            COUNT(*) FILTER (WHERE is_enabled = TRUE AND last_probe_status = 'reachable') AS reachable_total,
            COUNT(*) FILTER (WHERE is_enabled = TRUE AND last_probe_status = 'unreachable') AS unreachable_total,
            COUNT(*) FILTER (WHERE is_enabled = TRUE AND (last_probe_status IS NULL OR last_probe_status NOT IN ('reachable', 'unreachable'))) AS unknown_probe_total
         FROM monitoring_sources",
    )
    .fetch_one(db)
    .await?;

    let (status, summary, reason) = if row.enabled_total <= 0 {
        (
            "blocking",
            "No enabled monitoring source is configured.".to_string(),
            "Operators cannot rely on monitoring-driven workflows until at least one source is enabled."
                .to_string(),
        )
    } else if row.reachable_total <= 0 {
        (
            "blocking",
            "Monitoring sources are enabled but not proven reachable.".to_string(),
            "No enabled monitoring source has a successful probe result.".to_string(),
        )
    } else if row.unreachable_total > 0 || row.unknown_probe_total > 0 {
        (
            "warning",
            "Some monitoring coverage is degraded.".to_string(),
            "At least one enabled source is unreachable or has not completed a probe yet."
                .to_string(),
        )
    } else {
        (
            "ready",
            "Monitoring source coverage is healthy.".to_string(),
            "Enabled monitoring sources have successful probe results.".to_string(),
        )
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "monitoring_sources".to_string(),
        name: "Monitoring source readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action: Some(link_go_live_action(
            "go_live.monitoring_sources.review",
            "Review monitoring sources",
            "Open monitoring source configuration and probe state before production rollout.",
            "#/overview",
        )),
        evidence: json!({
            "enabled_total": row.enabled_total,
            "reachable_total": row.reachable_total,
            "unreachable_total": row.unreachable_total,
            "unknown_probe_total": row.unknown_probe_total,
        }),
    })
}

async fn build_operator_notifications_go_live_domain(
    db: &sqlx::PgPool,
) -> AppResult<GoLiveReadinessDomainItem> {
    let owner_suggestion: Option<GoLiveOwnerSuggestionRow> = sqlx::query_as(
        "SELECT owner_ref, notification_target
         FROM ops_runbook_risk_owner_directory
         WHERE is_enabled = TRUE
         ORDER BY display_name ASC, owner_key ASC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    let channel_suggestion: Option<GoLiveNotificationChannelSuggestionRow> = sqlx::query_as(
        "SELECT name, channel_type, target
         FROM discovery_notification_channels
         WHERE is_enabled = TRUE
         ORDER BY id ASC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    let template_enabled: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM discovery_notification_templates
            WHERE event_type = $1
              AND is_enabled = TRUE
        )",
    )
    .bind(GO_LIVE_NOTIFICATION_EVENT)
    .fetch_one(db)
    .await?;

    let row: NotificationGoLiveSummaryRow = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM discovery_notification_channels WHERE is_enabled = TRUE) AS enabled_channel_count,
            (
                SELECT COUNT(*)
                FROM discovery_notification_subscriptions
                WHERE event_type = $1
                  AND is_enabled = TRUE
            ) AS enabled_subscription_count",
    )
    .bind(GO_LIVE_NOTIFICATION_EVENT)
    .fetch_one(db)
    .await?;

    let (status, summary, reason) = if !template_enabled {
        (
            "blocking",
            "Runbook-risk notification template is missing.".to_string(),
            "Operator follow-up notifications cannot dispatch until the default template is enabled."
                .to_string(),
        )
    } else if row.enabled_channel_count <= 0 {
        (
            "blocking",
            "No enabled notification channel is configured.".to_string(),
            "The product has nowhere to send operator follow-up notifications.".to_string(),
        )
    } else if row.enabled_subscription_count <= 0 {
        (
            "blocking",
            "No enabled runbook-risk notification subscription exists.".to_string(),
            "The runbook-risk follow-up event is not wired to an enabled notification channel."
                .to_string(),
        )
    } else {
        (
            "ready",
            "Operator notification defaults are ready.".to_string(),
            "Template, channel, and subscription coverage exist for runbook-risk ticket follow-up."
                .to_string(),
        )
    };
    let suggested_target = channel_suggestion
        .as_ref()
        .map(|item| item.target.clone())
        .or_else(|| {
            owner_suggestion
                .as_ref()
                .and_then(|item| item.notification_target.clone())
        });
    let suggested_channel_name = channel_suggestion
        .as_ref()
        .map(|item| item.name.clone())
        .unwrap_or_else(|| "operator-bootstrap-primary".to_string());
    let suggested_channel_type = channel_suggestion
        .as_ref()
        .map(|item| item.channel_type.clone())
        .or_else(|| {
            suggested_target
                .as_deref()
                .map(infer_go_live_notification_channel_type)
        })
        .unwrap_or_else(|| "email".to_string());
    let recommended_action = if let Some(target) = suggested_target {
        Some(api_go_live_action(
            "go_live.operator_notifications.bootstrap",
            if status == "ready" {
                "Re-apply notification defaults"
            } else {
                "Apply notification defaults"
            },
            "Apply the default operator notification template, channel, and subscription for runbook-risk follow-up.",
            "/api/v1/ops/cockpit/integrations/bootstrap/apply",
            "POST",
            json!({
                "integration_key": "operator_notifications",
                "channel_name": suggested_channel_name,
                "channel_type": suggested_channel_type,
                "target": target
            }),
        ))
    } else {
        Some(blocked_api_go_live_action(
            "go_live.operator_notifications.bootstrap",
            "Prepare notification bootstrap",
            "Add an enabled owner notification_target first, then bootstrap operator notifications.",
            "Missing owner notification_target in runbook risk owner directory.",
            "#/overview",
        ))
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "operator_notifications".to_string(),
        name: "Operator notification readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action,
        evidence: json!({
            "event_type": GO_LIVE_NOTIFICATION_EVENT,
            "template_enabled": template_enabled,
            "enabled_channel_count": row.enabled_channel_count,
            "enabled_subscription_count": row.enabled_subscription_count,
        }),
    })
}

async fn build_ticket_followup_go_live_domain(
    db: &sqlx::PgPool,
) -> AppResult<GoLiveReadinessDomainItem> {
    let owner_suggestion: Option<GoLiveOwnerSuggestionRow> = sqlx::query_as(
        "SELECT owner_ref, notification_target
         FROM ops_runbook_risk_owner_directory
         WHERE is_enabled = TRUE
         ORDER BY display_name ASC, owner_key ASC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    let row: Option<TicketFollowupGoLiveRow> = sqlx::query_as(
        "SELECT is_enabled, escalate_to_assignee
         FROM ticket_escalation_policies
         WHERE policy_key = $1",
    )
    .bind(GO_LIVE_DEFAULT_TICKET_POLICY_KEY)
    .fetch_optional(db)
    .await?;

    let (status, summary, reason, evidence) = match &row {
        None => (
            "blocking",
            "Default ticket follow-up policy is missing.".to_string(),
            "Operators do not have a deterministic escalation owner for follow-up tickets."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_TICKET_POLICY_KEY,
                "policy_exists": false,
            }),
        ),
        Some(row) if !row.is_enabled => (
            "blocking",
            "Default ticket follow-up policy is disabled.".to_string(),
            "Ticket escalation rules exist but are not active.".to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_TICKET_POLICY_KEY,
                "policy_exists": true,
                "is_enabled": row.is_enabled,
                "escalate_to_assignee": row.escalate_to_assignee,
            }),
        ),
        Some(row) if row.escalate_to_assignee == GO_LIVE_FACTORY_ESCALATION_OWNER => (
            "blocking",
            "Default ticket follow-up owner still uses the factory placeholder.".to_string(),
            "Go-live requires an explicit enterprise escalation owner instead of the factory default."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_TICKET_POLICY_KEY,
                "policy_exists": true,
                "is_enabled": row.is_enabled,
                "escalate_to_assignee": row.escalate_to_assignee,
            }),
        ),
        Some(row) => (
            "ready",
            "Default ticket follow-up routing is ready.".to_string(),
            "A concrete escalation owner is configured for default ticket follow-up."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_TICKET_POLICY_KEY,
                "policy_exists": true,
                "is_enabled": row.is_enabled,
                "escalate_to_assignee": row.escalate_to_assignee,
            }),
        ),
    };
    let suggested_owner = match &row {
        Some(row)
            if row.is_enabled && row.escalate_to_assignee != GO_LIVE_FACTORY_ESCALATION_OWNER =>
        {
            Some(row.escalate_to_assignee.clone())
        }
        _ => owner_suggestion.as_ref().map(|item| item.owner_ref.clone()),
    };
    let recommended_action = if let Some(escalation_owner) = suggested_owner {
        Some(api_go_live_action(
            "go_live.ticket_followup.bootstrap",
            if status == "ready" {
                "Re-apply ticket follow-up defaults"
            } else {
                "Apply ticket follow-up defaults"
            },
            "Enable the default ticket follow-up policy with a concrete escalation owner.",
            "/api/v1/ops/cockpit/integrations/bootstrap/apply",
            "POST",
            json!({
                "integration_key": "ticket_followup_policy",
                "escalation_owner": escalation_owner
            }),
        ))
    } else {
        Some(blocked_api_go_live_action(
            "go_live.ticket_followup.bootstrap",
            "Prepare ticket follow-up bootstrap",
            "Add an enabled owner_ref first, then bootstrap default ticket follow-up routing.",
            "Missing enabled owner_ref suggestion in runbook risk owner directory.",
            "#/overview",
        ))
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "ticket_followup".to_string(),
        name: "Ticket follow-up readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action,
        evidence,
    })
}

async fn build_backup_restore_go_live_domain(
    db: &sqlx::PgPool,
) -> AppResult<GoLiveReadinessDomainItem> {
    let row: BackupGoLiveSummaryRow = sqlx::query_as(
        "SELECT
            COUNT(*) AS total_policies,
            COUNT(*) FILTER (WHERE last_backup_status IN ('never', 'failed')) AS stale_backup_policies,
            COUNT(*) FILTER (WHERE drill_enabled = TRUE AND last_drill_status IN ('never', 'failed')) AS drill_gap_policies,
            (SELECT COUNT(*) FROM ops_backup_restore_evidence WHERE closure_status = 'closed') AS closed_evidence_count,
            (SELECT COUNT(*) FROM ops_backup_restore_evidence WHERE closure_status = 'open') AS open_evidence_count
         FROM ops_backup_policies",
    )
    .fetch_one(db)
    .await?;

    let (status, summary, reason) = if row.total_policies <= 0 {
        (
            "blocking",
            "No backup policy is configured.".to_string(),
            "Operators do not have a baseline backup policy for go-live.".to_string(),
        )
    } else if row.stale_backup_policies > 0 {
        (
            "blocking",
            "At least one backup policy has never succeeded or last failed.".to_string(),
            "Go-live should not proceed until every configured backup policy has a successful recent run."
                .to_string(),
        )
    } else if row.closed_evidence_count <= 0 {
        (
            "warning",
            "Backup runs exist but restore evidence is still missing.".to_string(),
            "Operators have no closed restore verification evidence proving recoverability."
                .to_string(),
        )
    } else if row.drill_gap_policies > 0 || row.open_evidence_count > 0 {
        (
            "warning",
            "Backup coverage exists, but drill or evidence follow-up is incomplete.".to_string(),
            "At least one drill policy is stale or restore evidence remains open.".to_string(),
        )
    } else {
        (
            "ready",
            "Backup and restore evidence baseline is ready.".to_string(),
            "Configured backup policies have successful runs and closed restore verification evidence."
                .to_string(),
        )
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "backup_restore".to_string(),
        name: "Backup and restore readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action: Some(link_go_live_action(
            "go_live.backup_restore.review",
            "Review backup and restore readiness",
            "Open backup policies, runs, and restore evidence to close go-live gaps.",
            "#/overview",
        )),
        evidence: json!({
            "total_policies": row.total_policies,
            "stale_backup_policies": row.stale_backup_policies,
            "drill_gap_policies": row.drill_gap_policies,
            "closed_evidence_count": row.closed_evidence_count,
            "open_evidence_count": row.open_evidence_count,
        }),
    })
}

async fn build_runbook_execution_go_live_domain(
    db: &sqlx::PgPool,
) -> AppResult<GoLiveReadinessDomainItem> {
    let row: Option<RunbookExecutionGoLiveRow> = sqlx::query_as(
        "SELECT
            p.mode,
            jsonb_array_length(p.live_templates) AS live_template_count,
            (SELECT COUNT(*) FROM ops_runbook_execution_presets) AS preset_count
         FROM ops_runbook_execution_policies p
         WHERE p.policy_key = $1",
    )
    .bind(GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY)
    .fetch_optional(db)
    .await?;

    let (status, summary, reason, evidence) = match row {
        None => (
            "blocking",
            "Runbook execution policy is missing.".to_string(),
            "Operators do not have a seeded execution policy baseline.".to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY,
                "policy_exists": false,
            }),
        ),
        Some(row) if row.preset_count <= 0 => (
            "warning",
            "Runbook execution presets are not configured yet.".to_string(),
            "Operators can view templates, but they do not have reusable presets for guided execution."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY,
                "policy_exists": true,
                "mode": row.mode,
                "live_template_count": row.live_template_count,
                "preset_count": row.preset_count,
            }),
        ),
        Some(row) if row.mode != "hybrid_live" || row.live_template_count <= 0 => (
            "warning",
            "Runbook execution is limited to simulate-only or has no live-capable templates."
                .to_string(),
            "The platform can support guided runbooks, but production live execution is not fully enabled."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY,
                "policy_exists": true,
                "mode": row.mode,
                "live_template_count": row.live_template_count,
                "preset_count": row.preset_count,
            }),
        ),
        Some(row) => (
            "ready",
            "Runbook execution baseline is ready.".to_string(),
            "Operators have presets and a hybrid-live policy with live-capable templates."
                .to_string(),
            json!({
                "policy_key": GO_LIVE_DEFAULT_EXECUTION_POLICY_KEY,
                "policy_exists": true,
                "mode": row.mode,
                "live_template_count": row.live_template_count,
                "preset_count": row.preset_count,
            }),
        ),
    };

    Ok(GoLiveReadinessDomainItem {
        domain_key: "runbook_execution".to_string(),
        name: "Runbook execution readiness".to_string(),
        status: status.to_string(),
        summary,
        reason,
        recommended_action: Some(link_go_live_action(
            "go_live.runbook_execution.review",
            "Review runbook execution baseline",
            "Open runbook execution policy and presets before go-live.",
            "#/overview",
        )),
        evidence,
    })
}

fn summarize_go_live_readiness(domains: &[GoLiveReadinessDomainItem]) -> GoLiveReadinessSummary {
    let mut summary = GoLiveReadinessSummary {
        total: domains.len(),
        ready: 0,
        warning: 0,
        blocking: 0,
    };

    for item in domains {
        match item.status.as_str() {
            "ready" => summary.ready += 1,
            "warning" => summary.warning += 1,
            "blocking" => summary.blocking += 1,
            _ => {}
        }
    }

    summary
}

fn derive_go_live_overall_status(summary: &GoLiveReadinessSummary) -> String {
    if summary.blocking > 0 {
        "blocking".to_string()
    } else if summary.warning > 0 {
        "warning".to_string()
    } else {
        "ready".to_string()
    }
}

fn select_next_go_live_domain(domains: &[GoLiveReadinessDomainItem]) -> Option<String> {
    domains
        .iter()
        .find(|item| item.status == "blocking")
        .or_else(|| domains.iter().find(|item| item.status == "warning"))
        .map(|item| item.domain_key.clone())
}

fn link_go_live_action(
    action_key: &str,
    label: &str,
    description: &str,
    href: &str,
) -> GoLiveRemediationAction {
    GoLiveRemediationAction {
        action_key: action_key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        action_type: "link".to_string(),
        href: Some(href.to_string()),
        api_path: None,
        method: None,
        body: None,
        requires_write: false,
        auto_applicable: false,
        blocked_reason: None,
    }
}

fn api_go_live_action(
    action_key: &str,
    label: &str,
    description: &str,
    api_path: &str,
    method: &str,
    body: Value,
) -> GoLiveRemediationAction {
    GoLiveRemediationAction {
        action_key: action_key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        action_type: "api".to_string(),
        href: None,
        api_path: Some(api_path.to_string()),
        method: Some(method.to_string()),
        body: Some(body),
        requires_write: true,
        auto_applicable: true,
        blocked_reason: None,
    }
}

fn blocked_api_go_live_action(
    action_key: &str,
    label: &str,
    description: &str,
    blocked_reason: &str,
    href: &str,
) -> GoLiveRemediationAction {
    GoLiveRemediationAction {
        action_key: action_key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        action_type: "api".to_string(),
        href: Some(href.to_string()),
        api_path: None,
        method: None,
        body: None,
        requires_write: true,
        auto_applicable: false,
        blocked_reason: Some(blocked_reason.to_string()),
    }
}

fn infer_go_live_notification_channel_type(target: &str) -> String {
    let trimmed = target.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        "webhook".to_string()
    } else {
        "email".to_string()
    }
}

fn sort_next_best_actions(items: &mut [NextBestActionItem]) {
    items.sort_by(|left, right| {
        right
            .priority_score
            .cmp(&left.priority_score)
            .then_with(|| left.domain.cmp(&right.domain))
            .then_with(|| left.suggestion_key.cmp(&right.suggestion_key))
    });
}

async fn build_stale_stream_item(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<Option<DailyCockpitQueueItem>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT MAX(j.requested_at)
         FROM cmdb_monitoring_sync_jobs j
         INNER JOIN assets a ON a.id = j.asset_id
         WHERE 1=1",
    );

    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }

    let latest_event_at: Option<DateTime<Utc>> = builder.build_query_scalar().fetch_one(db).await?;
    let now = Utc::now();

    let lag_minutes = latest_event_at
        .map(|value| (now - value).num_minutes())
        .unwrap_or(STALE_STREAM_MINUTES + 1);

    if lag_minutes < STALE_STREAM_MINUTES {
        return Ok(None);
    }

    let score = (760 + lag_minutes.min(240) as i32).min(999);
    let scope_key = scope_key_label(site, department);

    Ok(Some(DailyCockpitQueueItem {
        queue_key: format!("stream-stale:{scope_key}"),
        item_type: "stream_stale".to_string(),
        priority_score: score,
        priority_level: if lag_minutes >= 60 {
            "critical".to_string()
        } else {
            "high".to_string()
        },
        rationale: format!(
            "Stream freshness lagged by {lag_minutes} minutes; prioritize source connectivity checks."
        ),
        rationale_details: vec![
            format!("lag_minutes:{lag_minutes}"),
            format!("threshold_minutes:{STALE_STREAM_MINUTES}"),
        ],
        observed_at: latest_event_at.unwrap_or(now),
        site: site.map(|value| value.to_string()),
        department: department.map(|value| value.to_string()),
        entity: json!({
            "latest_event_at": latest_event_at,
            "lag_minutes": lag_minutes,
            "scope_key": scope_key,
        }),
        actions: vec![
            DailyCockpitAction {
                key: "open-monitoring".to_string(),
                label: "Open Monitoring Workspace".to_string(),
                href: Some("#/monitoring".to_string()),
                api_path: None,
                method: None,
                body: None,
                requires_write: false,
            },
            DailyCockpitAction {
                key: "open-playbook-refresh-monitoring".to_string(),
                label: "Run Monitoring Refresh Playbook".to_string(),
                href: Some("#/workflow".to_string()),
                api_path: None,
                method: None,
                body: None,
                requires_write: true,
            },
        ],
    }))
}

fn build_alert_queue_items(rows: Vec<AlertQueueRow>) -> Vec<DailyCockpitQueueItem> {
    rows.into_iter()
        .map(|row| {
            let age_minutes = (Utc::now() - row.last_seen_at).num_minutes().max(0);
            let (priority_score, priority_level, rationale) =
                score_alert_item(&row.severity, &row.status, age_minutes);
            let mut rationale_details = vec![
                format!("severity:{}", row.severity),
                format!("status:{}", row.status),
                format!("age_minutes:{age_minutes}"),
            ];
            if let (Some(exec_id), Some(playbook_key), Some(mode), Some(status), Some(created_at)) = (
                row.remediation_execution_id,
                row.remediation_playbook_key.as_deref(),
                row.remediation_mode.as_deref(),
                row.remediation_status.as_deref(),
                row.remediation_created_at,
            ) {
                rationale_details.push(format!(
                    "latest_remediation:#{exec_id} {playbook_key} {mode}/{status} at {}",
                    created_at.to_rfc3339()
                ));
            }

            DailyCockpitQueueItem {
                queue_key: format!("alert:{}", row.id),
                item_type: "alert".to_string(),
                priority_score,
                priority_level,
                rationale,
                rationale_details,
                observed_at: row.last_seen_at,
                site: row.site.clone(),
                department: row.department.clone(),
                entity: json!({
                    "alert_id": row.id,
                    "alert_source": row.alert_source,
                    "alert_key": row.alert_key,
                    "title": row.title,
                    "severity": row.severity,
                    "status": row.status,
                    "asset_id": row.asset_id,
                    "latest_remediation": {
                        "execution_id": row.remediation_execution_id,
                        "playbook_key": row.remediation_playbook_key,
                        "mode": row.remediation_mode,
                        "status": row.remediation_status,
                        "created_at": row.remediation_created_at,
                    },
                }),
                actions: vec![
                    DailyCockpitAction {
                        key: "open-alert".to_string(),
                        label: "Open Alert Detail".to_string(),
                        href: Some("#/alerts".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: false,
                    },
                    DailyCockpitAction {
                        key: "ack-alert".to_string(),
                        label: "Acknowledge Alert".to_string(),
                        href: None,
                        api_path: Some(format!("/api/v1/alerts/{}/ack", row.id)),
                        method: Some("POST".to_string()),
                        body: Some(json!({ "note": "acknowledged from daily cockpit" })),
                        requires_write: true,
                    },
                    DailyCockpitAction {
                        key: "open-alert-remediation".to_string(),
                        label: "Run Alert Remediation".to_string(),
                        href: Some("#/alerts".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: true,
                    },
                ],
            }
        })
        .collect()
}

fn build_ticket_queue_items(rows: Vec<TicketQueueRow>) -> Vec<DailyCockpitQueueItem> {
    rows.into_iter()
        .map(|row| {
            let age_minutes = (Utc::now() - row.updated_at).num_minutes().max(0);
            let (priority_score, priority_level, rationale) =
                score_ticket_item(&row.priority, &row.status, age_minutes);

            DailyCockpitQueueItem {
                queue_key: format!("ticket:{}", row.id),
                item_type: "ticket".to_string(),
                priority_score,
                priority_level,
                rationale,
                rationale_details: vec![
                    format!("priority:{}", row.priority),
                    format!("status:{}", row.status),
                    format!("age_minutes:{age_minutes}"),
                ],
                observed_at: row.updated_at,
                site: row.site.clone(),
                department: row.department.clone(),
                entity: json!({
                    "ticket_id": row.id,
                    "ticket_no": row.ticket_no,
                    "title": row.title,
                    "status": row.status,
                    "priority": row.priority,
                    "assignee": row.assignee,
                }),
                actions: vec![
                    DailyCockpitAction {
                        key: "open-ticket".to_string(),
                        label: "Open Ticket".to_string(),
                        href: Some("#/tickets".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: false,
                    },
                    DailyCockpitAction {
                        key: "start-ticket".to_string(),
                        label: "Set In Progress".to_string(),
                        href: None,
                        api_path: Some(format!("/api/v1/tickets/{}/status", row.id)),
                        method: Some("PATCH".to_string()),
                        body: Some(json!({
                            "status": "in_progress",
                            "note": "updated from daily cockpit"
                        })),
                        requires_write: true,
                    },
                    DailyCockpitAction {
                        key: "open-playbook-remediation".to_string(),
                        label: "Open Remediation Playbook".to_string(),
                        href: Some("#/workflow".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: true,
                    },
                ],
            }
        })
        .collect()
}

fn build_sync_job_queue_items(rows: Vec<SyncJobQueueRow>) -> Vec<DailyCockpitQueueItem> {
    rows.into_iter()
        .map(|row| {
            let age_minutes = (Utc::now() - row.requested_at).num_minutes().max(0);
            let pending_stale = row.status == "pending";
            let (priority_score, priority_level, rationale) =
                score_sync_job_item(&row.status, age_minutes, pending_stale);

            DailyCockpitQueueItem {
                queue_key: format!("sync-job:{}", row.id),
                item_type: "sync_job".to_string(),
                priority_score,
                priority_level,
                rationale,
                rationale_details: vec![
                    format!("status:{}", row.status),
                    format!("attempt:{}/{}", row.attempt, row.max_attempts),
                    format!("age_minutes:{age_minutes}"),
                ],
                observed_at: row.requested_at,
                site: row.site.clone(),
                department: row.department.clone(),
                entity: json!({
                    "job_id": row.id,
                    "asset_id": row.asset_id,
                    "asset_name": row.asset_name,
                    "status": row.status,
                    "attempt": row.attempt,
                    "max_attempts": row.max_attempts,
                    "run_after": row.run_after,
                    "last_error": row.last_error,
                }),
                actions: vec![
                    DailyCockpitAction {
                        key: "open-topology".to_string(),
                        label: "Open Topology Context".to_string(),
                        href: Some("#/topology".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: false,
                    },
                    DailyCockpitAction {
                        key: "retry-monitoring-sync".to_string(),
                        label: "Retry Monitoring Sync".to_string(),
                        href: None,
                        api_path: Some(format!(
                            "/api/v1/cmdb/assets/{}/monitoring-sync",
                            row.asset_id
                        )),
                        method: Some("POST".to_string()),
                        body: Some(json!({ "reason": "daily cockpit retry" })),
                        requires_write: true,
                    },
                    DailyCockpitAction {
                        key: "open-playbook-refresh-monitoring".to_string(),
                        label: "Open Monitoring Refresh Playbook".to_string(),
                        href: Some("#/workflow".to_string()),
                        api_path: None,
                        method: None,
                        body: None,
                        requires_write: true,
                    },
                ],
            }
        })
        .collect()
}

fn score_alert_item(severity: &str, status: &str, age_minutes: i64) -> (i32, String, String) {
    let severity_score = match severity.trim().to_ascii_lowercase().as_str() {
        "critical" => 930,
        "warning" => 780,
        "info" => 620,
        _ => 540,
    };
    let status_score = match status.trim().to_ascii_lowercase().as_str() {
        "open" => 40,
        "acknowledged" => 10,
        _ => 0,
    };
    let age_boost = age_minutes.min(180) as i32;
    let score = (severity_score + status_score + age_boost).min(999);

    let level = if score >= 920 {
        "critical"
    } else if score >= 800 {
        "high"
    } else if score >= 680 {
        "medium"
    } else {
        "low"
    };

    (
        score,
        level.to_string(),
        format!(
            "{} alert is {} for {} minutes.",
            severity.trim().to_ascii_lowercase(),
            status.trim().to_ascii_lowercase(),
            age_minutes
        ),
    )
}

fn score_ticket_item(priority: &str, status: &str, age_minutes: i64) -> (i32, String, String) {
    let priority_score = match priority.trim().to_ascii_lowercase().as_str() {
        "critical" => 900,
        "high" => 820,
        "medium" => 710,
        "low" => 620,
        _ => 580,
    };
    let status_score = match status.trim().to_ascii_lowercase().as_str() {
        "open" => 35,
        "in_progress" => 15,
        _ => 0,
    };
    let age_boost = age_minutes.min(120) as i32;
    let score = (priority_score + status_score + age_boost).min(999);

    let level = if score >= 900 {
        "critical"
    } else if score >= 780 {
        "high"
    } else if score >= 680 {
        "medium"
    } else {
        "low"
    };

    (
        score,
        level.to_string(),
        format!(
            "{} ticket has status {} and has not been updated for {} minutes.",
            priority.trim().to_ascii_lowercase(),
            status.trim().to_ascii_lowercase(),
            age_minutes
        ),
    )
}

fn score_sync_job_item(
    status: &str,
    age_minutes: i64,
    pending_stale: bool,
) -> (i32, String, String) {
    let base = match status.trim().to_ascii_lowercase().as_str() {
        "dead_letter" => 920,
        "failed" => 840,
        "pending" if pending_stale => 720,
        _ => 620,
    };
    let score = (base + age_minutes.min(120) as i32).min(999);

    let level = if score >= 920 {
        "critical"
    } else if score >= 800 {
        "high"
    } else if score >= 680 {
        "medium"
    } else {
        "low"
    };

    (
        score,
        level.to_string(),
        format!(
            "Sync job status '{}' has aged {} minutes and needs intervention.",
            status.trim().to_ascii_lowercase(),
            age_minutes
        ),
    )
}

async fn collect_daily_ops_candidates(
    state: &AppState,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
    now: DateTime<Utc>,
) -> AppResult<Vec<DailyOpsCandidateItem>> {
    let mut items = Vec::new();
    let escalation_window = load_default_escalation_window(&state.db).await?;

    let alert_items =
        build_alert_queue_items(fetch_alert_queue_rows(&state.db, site, department, limit).await?)
            .into_iter()
            .map(|item| build_daily_ops_candidate_from_cockpit_item(item, &escalation_window, now))
            .collect::<Vec<_>>();
    items.extend(alert_items);

    let ticket_items = build_ticket_queue_items(
        fetch_ticket_queue_rows(&state.db, site, department, limit).await?,
    )
    .into_iter()
    .map(|item| build_daily_ops_candidate_from_cockpit_item(item, &escalation_window, now))
    .collect::<Vec<_>>();
    items.extend(ticket_items);

    items.extend(
        collect_daily_ops_runbook_risk_items(
            &state.db,
            escalation_window.escalate_to_assignee.as_str(),
            now,
        )
        .await?,
    );
    items.extend(
        collect_go_live_readiness_domains(state)
            .await?
            .into_iter()
            .filter(|item| item.status != "ready")
            .map(|item| {
                build_daily_ops_candidate_from_go_live_domain(
                    item,
                    escalation_window.escalate_to_assignee.as_str(),
                    now,
                )
            }),
    );
    items.extend(
        collect_daily_ops_activation_items(
            &state.db,
            escalation_window.escalate_to_assignee.as_str(),
            now,
        )
        .await?,
    );

    Ok(items)
}

fn build_daily_ops_candidate_from_cockpit_item(
    item: DailyCockpitQueueItem,
    escalation_window: &EscalationWindowRow,
    now: DateTime<Utc>,
) -> DailyOpsCandidateItem {
    let task_key = item.queue_key;
    let item_type = item.item_type;
    let summary = match item_type.as_str() {
        "alert" => item
            .entity
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Open alert requires follow-up.")
            .to_string(),
        "ticket" => item
            .entity
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Ticket requires follow-up.")
            .to_string(),
        other => format!("{} requires operator follow-up.", other.replace('_', " ")),
    };
    let due_policy =
        resolve_due_policy_for_cockpit_item(&item_type, &item.priority_level, escalation_window);
    let due_at =
        Some(item.observed_at + chrono::Duration::minutes(due_policy.due_window_minutes as i64));
    let escalate_at = Some(
        item.observed_at + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64),
    );
    let owner = resolve_owner_for_cockpit_item(
        &item_type,
        &item.entity,
        escalation_window.escalate_to_assignee.as_str(),
    );
    let base_status = if due_at.map(|value| value <= now).unwrap_or(false) {
        "overdue".to_string()
    } else {
        "due_today".to_string()
    };
    let recommended_action = select_daily_ops_recommended_action(
        item.actions.as_slice(),
        match item_type.as_str() {
            "alert" => Some("open-alert"),
            "ticket" => Some("open-ticket"),
            _ => None,
        },
        &summary,
    );

    DailyOpsCandidateItem {
        task_key,
        item_type: item_type.clone(),
        domain: item_type,
        base_status,
        priority: item.priority_level,
        owner,
        summary,
        reason: item.rationale,
        recommended_action,
        due_at,
        escalate_at,
        due_policy,
        observed_at: item.observed_at,
        evidence: item.entity,
    }
}

fn resolve_due_policy_for_cockpit_item(
    item_type: &str,
    priority: &str,
    escalation_window: &EscalationWindowRow,
) -> DailyOpsDuePolicy {
    match item_type {
        "ticket" => {
            let (due, escalation) = escalation_threshold_for_priority(escalation_window, priority);
            DailyOpsDuePolicy {
                policy_key: GO_LIVE_DEFAULT_TICKET_POLICY_KEY.to_string(),
                due_window_minutes: due,
                escalation_window_minutes: escalation,
                source: "ticket_escalation_policies.default-ticket-sla".to_string(),
            }
        }
        "alert" => {
            let (due, escalation) = match priority {
                "critical" => (30, 60),
                "high" => (120, 240),
                "medium" => (360, 720),
                _ => (720, 1_440),
            };
            DailyOpsDuePolicy {
                policy_key: "alert-priority-response".to_string(),
                due_window_minutes: due,
                escalation_window_minutes: escalation,
                source: "built_in.alert-priority-response".to_string(),
            }
        }
        _ => DailyOpsDuePolicy {
            policy_key: "daily-ops-default".to_string(),
            due_window_minutes: 1_440,
            escalation_window_minutes: 2_880,
            source: "built_in.daily-ops-default".to_string(),
        },
    }
}

fn resolve_owner_for_cockpit_item(
    item_type: &str,
    entity: &Value,
    fallback_owner: &str,
) -> DailyOpsOwner {
    if item_type == "ticket" {
        if let Some(owner_ref) = entity
            .get("assignee")
            .and_then(Value::as_str)
            .map(str::trim)
        {
            if !owner_ref.is_empty() {
                return DailyOpsOwner {
                    owner_ref: Some(owner_ref.to_string()),
                    owner_state: "assigned".to_string(),
                    source: "ticket.assignee".to_string(),
                    reason: "Ticket already has an assignee.".to_string(),
                };
            }
        }
        return DailyOpsOwner {
            owner_ref: Some(fallback_owner.to_string()),
            owner_state: "assigned".to_string(),
            source: "ticket_escalation_policies.default-ticket-sla".to_string(),
            reason: "Ticket assignee is empty; default escalation owner applied.".to_string(),
        };
    }

    if fallback_owner.trim().is_empty() {
        return DailyOpsOwner {
            owner_ref: None,
            owner_state: "owner_gap".to_string(),
            source: "daily-ops-default".to_string(),
            reason: "No deterministic owner could be derived.".to_string(),
        };
    }

    DailyOpsOwner {
        owner_ref: Some(fallback_owner.to_string()),
        owner_state: "assigned".to_string(),
        source: "ticket_escalation_policies.default-ticket-sla".to_string(),
        reason: "Default escalation owner applied.".to_string(),
    }
}

fn select_daily_ops_recommended_action(
    actions: &[DailyCockpitAction],
    preferred_key: Option<&str>,
    summary: &str,
) -> Option<DailyOpsRecommendedAction> {
    let selected = preferred_key
        .and_then(|key| actions.iter().find(|item| item.key == key))
        .or_else(|| actions.iter().find(|item| item.href.is_some()))
        .or_else(|| actions.first())?;

    Some(DailyOpsRecommendedAction {
        action_key: selected.key.clone(),
        label: selected.label.clone(),
        description: summary.to_string(),
        action_type: if selected.href.is_some() {
            "link".to_string()
        } else {
            "api".to_string()
        },
        href: selected.href.clone(),
        api_path: selected.api_path.clone(),
        method: selected.method.clone(),
        body: selected.body.clone(),
        requires_write: selected.requires_write,
    })
}

fn build_daily_ops_candidate_from_go_live_domain(
    item: GoLiveReadinessDomainItem,
    default_owner: &str,
    now: DateTime<Utc>,
) -> DailyOpsCandidateItem {
    let priority = if item.status == "blocking" {
        "critical".to_string()
    } else {
        "high".to_string()
    };
    let due_policy = DailyOpsDuePolicy {
        policy_key: "go-live-readiness".to_string(),
        due_window_minutes: if item.status == "blocking" {
            0
        } else {
            12 * 60
        },
        escalation_window_minutes: if item.status == "blocking" {
            60
        } else {
            24 * 60
        },
        source: "built_in.go-live-readiness".to_string(),
    };
    let due_at = Some(now + chrono::Duration::minutes(due_policy.due_window_minutes as i64));
    let escalate_at =
        Some(now + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64));
    DailyOpsCandidateItem {
        task_key: format!("go-live:{}", item.domain_key),
        item_type: "go_live".to_string(),
        domain: "go_live".to_string(),
        base_status: if item.status == "blocking" {
            "blocked".to_string()
        } else {
            "due_today".to_string()
        },
        priority,
        owner: DailyOpsOwner {
            owner_ref: Some(default_owner.to_string()),
            owner_state: "assigned".to_string(),
            source: "ticket_escalation_policies.default-ticket-sla".to_string(),
            reason: "Go-live follow-up defaults to the escalation owner.".to_string(),
        },
        summary: item.summary,
        reason: item.reason,
        recommended_action: item
            .recommended_action
            .map(|action| DailyOpsRecommendedAction {
                action_key: action.action_key,
                label: action.label,
                description: action.description,
                action_type: action.action_type,
                href: action.href,
                api_path: action.api_path,
                method: action.method,
                body: action.body,
                requires_write: action.requires_write,
            }),
        due_at,
        escalate_at,
        due_policy,
        observed_at: now,
        evidence: json!({
            "domain_key": item.domain_key,
            "status": item.status,
            "evidence": item.evidence,
        }),
    }
}

async fn collect_daily_ops_runbook_risk_items(
    db: &sqlx::PgPool,
    default_owner: &str,
    now: DateTime<Utc>,
) -> AppResult<Vec<DailyOpsCandidateItem>> {
    let start_at = now - chrono::Duration::days(14);
    let owner_route_map = load_daily_ops_runbook_owner_route_map(db).await?;
    let rows: Vec<DailyOpsRunbookRiskAggregateRow> = sqlx::query_as(
        "SELECT template_key,
                MAX(template_name) AS template_name,
                COUNT(*)::bigint AS executions,
                COUNT(*) FILTER (WHERE status = 'failed')::bigint AS failed,
                MAX(created_at) FILTER (WHERE status = 'failed') AS latest_failed_at
         FROM ops_runbook_template_executions
         WHERE created_at >= $1
         GROUP BY template_key
         HAVING COUNT(*) FILTER (WHERE status = 'failed') > 0
         ORDER BY failed DESC, latest_failed_at DESC, template_key ASC
         LIMIT 20",
    )
    .bind(start_at)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let failure_rate = if row.executions <= 0 {
                0.0
            } else {
                (row.failed as f64 / row.executions as f64) * 100.0
            };
            let (priority, due_window_hours, escalation_window_hours) =
                if row.failed >= 3 || failure_rate >= 50.0 {
                    ("critical".to_string(), 6, 12)
                } else if row.failed >= 2 || failure_rate >= 25.0 {
                    ("high".to_string(), 12, 24)
                } else {
                    ("medium".to_string(), 24, 48)
                };
            let due_policy = DailyOpsDuePolicy {
                policy_key: "runbook-risk-failure-rate".to_string(),
                due_window_minutes: due_window_hours * 60,
                escalation_window_minutes: escalation_window_hours * 60,
                source: "built_in.runbook-risk-failure-rate".to_string(),
            };
            let (owner_ref, owner_source, owner_reason) =
                if let Some(owner) = owner_route_map.get(row.template_key.as_str()) {
                    (
                        owner.clone(),
                        "ops_runbook_risk_owner_routing_rules".to_string(),
                        "Owner resolved from enabled runbook-risk owner routing rule.".to_string(),
                    )
                } else {
                    (
                        default_owner.to_string(),
                        "ticket_escalation_policies.default-ticket-sla".to_string(),
                        "No enabled route found; fallback to default escalation owner.".to_string(),
                    )
                };
            let due_at =
                row.latest_failed_at + chrono::Duration::minutes(due_policy.due_window_minutes as i64);
            let escalate_at = row.latest_failed_at
                + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64);
            DailyOpsCandidateItem {
                task_key: format!("runbook-risk:{}", row.template_key),
                item_type: "runbook_risk".to_string(),
                domain: "runbook_risk".to_string(),
                base_status: if due_at <= now {
                    "overdue".to_string()
                } else {
                    "due_today".to_string()
                },
                priority,
                owner: DailyOpsOwner {
                    owner_ref: Some(owner_ref.clone()),
                    owner_state: "assigned".to_string(),
                    source: owner_source,
                    reason: owner_reason,
                },
                summary: format!("{} has repeated failed executions.", row.template_name),
                reason: format!(
                    "Failure rate is {:.0}% with {} failed run(s) from {} execution(s) in the last 14 days.",
                    failure_rate, row.failed, row.executions
                ),
                recommended_action: Some(DailyOpsRecommendedAction {
                    action_key: "open-runbook-risk".to_string(),
                    label: "Review Runbook Risk".to_string(),
                    description: "Inspect failure hotspots and decide whether the template needs routing, replay, or policy adjustment.".to_string(),
                    action_type: "link".to_string(),
                    href: Some("#/overview".to_string()),
                    api_path: None,
                    method: None,
                    body: None,
                    requires_write: false,
                }),
                due_at: Some(due_at),
                escalate_at: Some(escalate_at),
                due_policy,
                observed_at: row.latest_failed_at,
                evidence: json!({
                    "template_key": row.template_key,
                    "template_name": row.template_name,
                    "owner_ref": owner_ref,
                    "executions": row.executions,
                    "failed": row.failed,
                    "failure_rate_percent": failure_rate,
                    "latest_failed_at": row.latest_failed_at,
                }),
            }
        })
        .collect())
}

async fn load_daily_ops_runbook_owner_route_map(
    db: &sqlx::PgPool,
) -> AppResult<HashMap<String, String>> {
    let rows: Vec<RunbookRiskOwnerRouteRow> = sqlx::query_as(
        "SELECT DISTINCT ON (rule.template_key)
            rule.template_key,
            directory.owner_ref
         FROM ops_runbook_risk_owner_routing_rules rule
         INNER JOIN ops_runbook_risk_owner_directory directory
                 ON directory.owner_key = rule.owner_key
         WHERE rule.is_enabled = TRUE
           AND directory.is_enabled = TRUE
         ORDER BY rule.template_key ASC, rule.priority ASC, rule.updated_at DESC, rule.id DESC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.template_key, row.owner_ref))
        .collect())
}

async fn collect_daily_ops_activation_items(
    db: &sqlx::PgPool,
    default_owner: &str,
    now: DateTime<Utc>,
) -> AppResult<Vec<DailyOpsCandidateItem>> {
    let applied_profiles: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM setup_operator_profile_runs WHERE status = 'applied'",
    )
    .fetch_one(db)
    .await?;
    let applied_templates: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM setup_bootstrap_template_runs WHERE status = 'applied'",
    )
    .fetch_one(db)
    .await?;
    let latest_feedback: Option<DailyOpsActivationFeedbackRow> = sqlx::query_as(
        "SELECT step_key, template_key, feedback_kind, comment, created_at
         FROM setup_activation_feedback_events
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;

    let mut items = Vec::new();
    if applied_profiles <= 0 {
        let due_policy = DailyOpsDuePolicy {
            policy_key: "activation-profile-adoption".to_string(),
            due_window_minutes: 12 * 60,
            escalation_window_minutes: 24 * 60,
            source: "built_in.activation-profile-adoption".to_string(),
        };
        items.push(DailyOpsCandidateItem {
            task_key: "activation:operator_profile".to_string(),
            item_type: "activation".to_string(),
            domain: "activation".to_string(),
            base_status: "due_today".to_string(),
            priority: "high".to_string(),
            owner: DailyOpsOwner {
                owner_ref: Some(default_owner.to_string()),
                owner_state: "assigned".to_string(),
                source: "ticket_escalation_policies.default-ticket-sla".to_string(),
                reason: "Activation tasks route to the default escalation owner.".to_string(),
            },
            summary: "Recommended SMB operator profile has not been applied.".to_string(),
            reason: "Operators still lack the baseline profile that turns activation guidance into reusable defaults.".to_string(),
            recommended_action: Some(DailyOpsRecommendedAction {
                action_key: "open-setup-activation".to_string(),
                label: "Open Setup Activation".to_string(),
                description: "Apply the recommended activation profile so the environment has an operator-safe baseline.".to_string(),
                action_type: "link".to_string(),
                href: Some("#/setup".to_string()),
                api_path: None,
                method: None,
                body: None,
                requires_write: true,
            }),
            due_at: Some(now + chrono::Duration::minutes(due_policy.due_window_minutes as i64)),
            escalate_at: Some(
                now + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64),
            ),
            due_policy,
            observed_at: now,
            evidence: json!({
                "applied_profile_runs": applied_profiles,
            }),
        });
    }
    if applied_templates <= 0 {
        let due_policy = DailyOpsDuePolicy {
            policy_key: "activation-template-adoption".to_string(),
            due_window_minutes: 18 * 60,
            escalation_window_minutes: 30 * 60,
            source: "built_in.activation-template-adoption".to_string(),
        };
        items.push(DailyOpsCandidateItem {
            task_key: "activation:template-baseline".to_string(),
            item_type: "activation".to_string(),
            domain: "activation".to_string(),
            base_status: "due_today".to_string(),
            priority: "medium".to_string(),
            owner: DailyOpsOwner {
                owner_ref: Some(default_owner.to_string()),
                owner_state: "assigned".to_string(),
                source: "ticket_escalation_policies.default-ticket-sla".to_string(),
                reason: "Activation tasks route to the default escalation owner.".to_string(),
            },
            summary: "No setup template baseline has been applied yet.".to_string(),
            reason: "Activation still depends on blank-page configuration instead of reusable starter templates.".to_string(),
            recommended_action: Some(DailyOpsRecommendedAction {
                action_key: "open-setup-templates".to_string(),
                label: "Review Starter Templates".to_string(),
                description: "Apply one starter template so monitoring, ticketing, and notification defaults are visible in product.".to_string(),
                action_type: "link".to_string(),
                href: Some("#/setup".to_string()),
                api_path: None,
                method: None,
                body: None,
                requires_write: true,
            }),
            due_at: Some(now + chrono::Duration::minutes(due_policy.due_window_minutes as i64)),
            escalate_at: Some(
                now + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64),
            ),
            due_policy,
            observed_at: now,
            evidence: json!({
                "applied_template_runs": applied_templates,
            }),
        });
    }
    if let Some(feedback) = latest_feedback.filter(|item| item.feedback_kind != "not_applicable") {
        let due_policy = DailyOpsDuePolicy {
            policy_key: "activation-feedback-blocked".to_string(),
            due_window_minutes: 0,
            escalation_window_minutes: 6 * 60,
            source: "built_in.activation-feedback-blocked".to_string(),
        };
        items.push(DailyOpsCandidateItem {
            task_key: format!("activation-feedback:{}", feedback.step_key),
            item_type: "activation_feedback".to_string(),
            domain: "activation".to_string(),
            base_status: "blocked".to_string(),
            priority: "high".to_string(),
            owner: DailyOpsOwner {
                owner_ref: Some(default_owner.to_string()),
                owner_state: "assigned".to_string(),
                source: "ticket_escalation_policies.default-ticket-sla".to_string(),
                reason: "Blocked activation feedback routes to the default escalation owner."
                    .to_string(),
            },
            summary: "Recent activation feedback reports unresolved friction.".to_string(),
            reason: feedback
                .comment
                .clone()
                .unwrap_or_else(|| format!("Activation step '{}' is marked as {}.", feedback.step_key, feedback.feedback_kind)),
            recommended_action: Some(DailyOpsRecommendedAction {
                action_key: "review-activation-feedback".to_string(),
                label: "Review Activation Feedback".to_string(),
                description: "Inspect the blocked or confusing activation step before expecting daily return usage.".to_string(),
                action_type: "link".to_string(),
                href: Some("#/setup".to_string()),
                api_path: None,
                method: None,
                body: None,
                requires_write: false,
            }),
            due_at: Some(feedback.created_at),
            escalate_at: Some(
                feedback.created_at
                    + chrono::Duration::minutes(due_policy.escalation_window_minutes as i64),
            ),
            due_policy,
            observed_at: feedback.created_at,
            evidence: json!({
                "step_key": feedback.step_key,
                "template_key": feedback.template_key,
                "feedback_kind": feedback.feedback_kind,
                "comment": feedback.comment,
            }),
        });
    }

    Ok(items)
}

async fn load_daily_ops_follow_up_states(
    db: &sqlx::PgPool,
    task_keys: &[String],
) -> AppResult<HashMap<String, DailyOpsFollowUpStateRow>> {
    if task_keys.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<DailyOpsFollowUpStateRow> = sqlx::query_as(
        "SELECT task_key, item_type, follow_up_state, note, defer_until, actor, updated_at
         FROM ops_daily_follow_up_states
         WHERE task_key = ANY($1)",
    )
    .bind(task_keys)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.task_key.clone(), row))
        .collect())
}

fn build_daily_ops_follow_up_item(
    candidate: DailyOpsCandidateItem,
    state: Option<&DailyOpsFollowUpStateRow>,
    now: DateTime<Utc>,
) -> DailyOpsFollowUpItem {
    let follow_up_state = state
        .map(|item| item.follow_up_state.clone())
        .unwrap_or_else(|| "new".to_string());
    let deferred_until = state.and_then(|item| item.defer_until);
    let status = derive_daily_ops_status(candidate.base_status.as_str(), state, now);
    let mut evidence = candidate.evidence;
    evidence["owner"] = json!({
        "owner_ref": candidate.owner.owner_ref.clone(),
        "owner_state": candidate.owner.owner_state.clone(),
        "source": candidate.owner.source.clone(),
        "reason": candidate.owner.reason.clone(),
    });
    evidence["due_policy"] = json!({
        "policy_key": candidate.due_policy.policy_key.clone(),
        "due_window_minutes": candidate.due_policy.due_window_minutes,
        "escalation_window_minutes": candidate.due_policy.escalation_window_minutes,
        "source": candidate.due_policy.source.clone(),
    });
    evidence["escalate_at"] = candidate
        .escalate_at
        .map(|value| Value::String(value.to_rfc3339()))
        .unwrap_or(Value::Null);
    if let Some(row) = state {
        evidence["follow_up_note"] = row.note.clone().map(Value::String).unwrap_or(Value::Null);
        evidence["follow_up_actor"] = Value::String(row.actor.clone());
        evidence["follow_up_updated_at"] = Value::String(row.updated_at.to_rfc3339());
    }

    DailyOpsFollowUpItem {
        task_key: candidate.task_key,
        item_type: candidate.item_type,
        domain: candidate.domain,
        status,
        follow_up_state: follow_up_state.clone(),
        priority: candidate.priority,
        owner: candidate.owner,
        summary: candidate.summary,
        reason: candidate.reason,
        recommended_action: candidate.recommended_action,
        available_actions: build_daily_ops_task_actions(),
        due_at: candidate.due_at,
        escalate_at: candidate.escalate_at,
        due_policy: candidate.due_policy,
        observed_at: candidate.observed_at,
        acknowledged_at: state
            .filter(|item| item.follow_up_state == DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED)
            .map(|item| item.updated_at),
        completed_at: state
            .filter(|item| item.follow_up_state == DAILY_OPS_FOLLOW_UP_STATE_COMPLETED)
            .map(|item| item.updated_at),
        deferred_until,
        evidence,
    }
}

fn derive_daily_ops_status(
    base_status: &str,
    state: Option<&DailyOpsFollowUpStateRow>,
    now: DateTime<Utc>,
) -> String {
    match state.map(|item| item.follow_up_state.as_str()) {
        Some(DAILY_OPS_FOLLOW_UP_STATE_COMPLETED) => "completed".to_string(),
        Some(DAILY_OPS_FOLLOW_UP_STATE_DEFERRED)
            if state
                .and_then(|item| item.defer_until)
                .map(|value| value > now)
                .unwrap_or(false) =>
        {
            "deferred".to_string()
        }
        _ => base_status.to_string(),
    }
}

fn build_daily_ops_task_actions() -> Vec<DailyOpsTaskAction> {
    vec![
        DailyOpsTaskAction {
            action_key: "acknowledge".to_string(),
            label: "Acknowledge".to_string(),
            requires_write: true,
        },
        DailyOpsTaskAction {
            action_key: "complete".to_string(),
            label: "Complete".to_string(),
            requires_write: true,
        },
        DailyOpsTaskAction {
            action_key: "defer".to_string(),
            label: "Defer".to_string(),
            requires_write: true,
        },
    ]
}

fn summarize_daily_ops_items(items: &[DailyOpsFollowUpItem]) -> DailyOpsBriefingSummary {
    let mut summary = DailyOpsBriefingSummary::default();
    summary.total = items.len();
    for item in items {
        match item.status.as_str() {
            "due_today" => summary.due_today += 1,
            "overdue" => summary.overdue += 1,
            "blocked" => summary.blocked += 1,
            "completed" => summary.completed += 1,
            "deferred" => summary.deferred += 1,
            _ => {}
        }
        if item.follow_up_state == DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED {
            summary.acknowledged += 1;
        }
        match item.priority.as_str() {
            "critical" => summary.critical += 1,
            "high" => summary.high += 1,
            "medium" => summary.medium += 1,
            _ => summary.low += 1,
        }
    }
    summary
}

fn select_recommended_daily_ops_task(items: &[DailyOpsFollowUpItem]) -> Option<String> {
    items.first().map(|item| item.task_key.clone())
}

fn sort_daily_ops_items(items: &mut [DailyOpsFollowUpItem]) {
    items.sort_by(|left, right| {
        daily_ops_status_rank(right.status.as_str())
            .cmp(&daily_ops_status_rank(left.status.as_str()))
            .then_with(|| {
                daily_ops_priority_rank(right.priority.as_str())
                    .cmp(&daily_ops_priority_rank(left.priority.as_str()))
            })
            .then_with(|| {
                let left_due = left.due_at.unwrap_or(left.observed_at);
                let right_due = right.due_at.unwrap_or(right.observed_at);
                left_due.cmp(&right_due)
            })
            .then_with(|| right.observed_at.cmp(&left.observed_at))
            .then_with(|| left.task_key.cmp(&right.task_key))
    });
}

fn daily_ops_status_rank(status: &str) -> i32 {
    match status {
        "blocked" => 5,
        "overdue" => 4,
        "due_today" => 3,
        "deferred" => 2,
        "completed" => 1,
        _ => 0,
    }
}

fn daily_ops_priority_rank(priority: &str) -> i32 {
    match priority {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

fn normalize_daily_ops_action(raw: String) -> AppResult<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "acknowledge" | "complete" | "defer" => Ok(normalized),
        _ => Err(AppError::Validation(
            "action must be one of: acknowledge, complete, defer".to_string(),
        )),
    }
}

fn normalize_daily_ops_follow_up_note(raw: Option<String>) -> AppResult<Option<String>> {
    match raw {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.len() > MAX_DAILY_OPS_NOTE_LEN {
                Err(AppError::Validation(format!(
                    "note must be <= {MAX_DAILY_OPS_NOTE_LEN} characters"
                )))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        None => Ok(None),
    }
}

fn parse_daily_ops_defer_until(
    raw: Option<String>,
    action: &str,
) -> AppResult<Option<DateTime<Utc>>> {
    match (action, raw) {
        ("defer", None) => Err(AppError::Validation(
            "defer_until is required when action=defer".to_string(),
        )),
        ("defer", Some(value)) => DateTime::parse_from_rfc3339(value.trim())
            .map(|item| item.with_timezone(&Utc))
            .map(Some)
            .map_err(|_| AppError::Validation("defer_until must be RFC3339".to_string())),
        (_, None) => Ok(None),
        (_, Some(value)) if value.trim().is_empty() => Ok(None),
        (_, Some(value)) => DateTime::parse_from_rfc3339(value.trim())
            .map(|item| Some(item.with_timezone(&Utc)))
            .map_err(|_| AppError::Validation("defer_until must be RFC3339".to_string())),
    }
}

async fn upsert_daily_ops_follow_up_state(
    db: &sqlx::PgPool,
    task_key: &str,
    item_type: &str,
    action: &str,
    note: Option<String>,
    defer_until: Option<DateTime<Utc>>,
    actor: &str,
) -> AppResult<DailyOpsFollowUpStateRow> {
    let follow_up_state = match action {
        "acknowledge" => DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED,
        "complete" => DAILY_OPS_FOLLOW_UP_STATE_COMPLETED,
        "defer" => DAILY_OPS_FOLLOW_UP_STATE_DEFERRED,
        _ => {
            return Err(AppError::Validation(
                "action must be one of: acknowledge, complete, defer".to_string(),
            ));
        }
    };
    let metadata = json!({
        "action": action,
    });

    let row = sqlx::query_as(
        "INSERT INTO ops_daily_follow_up_states (
            task_key, item_type, follow_up_state, note, defer_until, actor, metadata
         ) VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (task_key) DO UPDATE
         SET item_type = EXCLUDED.item_type,
             follow_up_state = EXCLUDED.follow_up_state,
             note = EXCLUDED.note,
             defer_until = EXCLUDED.defer_until,
             actor = EXCLUDED.actor,
             metadata = EXCLUDED.metadata,
             updated_at = NOW()
         RETURNING task_key, item_type, follow_up_state, note, defer_until, actor, updated_at",
    )
    .bind(task_key)
    .bind(item_type)
    .bind(follow_up_state)
    .bind(note)
    .bind(defer_until)
    .bind(actor)
    .bind(metadata)
    .fetch_one(db)
    .await?;

    Ok(row)
}

fn sort_daily_queue_items(items: &mut [DailyCockpitQueueItem]) {
    items.sort_by(|left, right| {
        right
            .priority_score
            .cmp(&left.priority_score)
            .then_with(|| right.observed_at.cmp(&left.observed_at))
            .then_with(|| left.queue_key.cmp(&right.queue_key))
    });
}

fn scope_key_label(site: Option<&str>, department: Option<&str>) -> String {
    match (site, department) {
        (Some(site), Some(department)) => format!("site:{site}|department:{department}"),
        (Some(site), None) => format!("site:{site}"),
        (None, Some(department)) => format!("department:{department}"),
        (None, None) => "global".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Duration, NaiveDate, Utc};
    use serde_json::json;

    use super::{
        DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED, DAILY_OPS_FOLLOW_UP_STATE_COMPLETED,
        DailyCockpitAction, DailyCockpitQueueItem, DailyOpsDuePolicy, DailyOpsFollowUpItem,
        DailyOpsFollowUpStateRow, DailyOpsOwner, GoLiveReadinessDomainItem, MAX_CHECKLIST_NOTE_LEN,
        NextBestActionItem, OpsChecklistEntryRow, OpsChecklistTemplateRow,
        build_ops_checklist_response, derive_daily_ops_status, derive_go_live_overall_status,
        normalize_daily_ops_action, normalize_optional_note, parse_optional_date, score_alert_item,
        score_ticket_item, select_next_go_live_domain, sort_daily_ops_items,
        sort_daily_queue_items, sort_next_best_actions, summarize_daily_ops_items,
        summarize_go_live_readiness,
    };

    fn test_item(key: &str, score: i32, observed_at_offset_minutes: i64) -> DailyCockpitQueueItem {
        DailyCockpitQueueItem {
            queue_key: key.to_string(),
            item_type: "test".to_string(),
            priority_score: score,
            priority_level: "medium".to_string(),
            rationale: "test".to_string(),
            rationale_details: vec![],
            observed_at: Utc::now() - Duration::minutes(observed_at_offset_minutes),
            site: None,
            department: None,
            entity: serde_json::json!({}),
            actions: vec![DailyCockpitAction {
                key: "noop".to_string(),
                label: "noop".to_string(),
                href: None,
                api_path: None,
                method: None,
                body: None,
                requires_write: false,
            }],
        }
    }

    fn test_daily_ops_owner() -> DailyOpsOwner {
        DailyOpsOwner {
            owner_ref: Some("ops-escalation".to_string()),
            owner_state: "assigned".to_string(),
            source: "test".to_string(),
            reason: "test fixture".to_string(),
        }
    }

    fn test_daily_ops_due_policy() -> DailyOpsDuePolicy {
        DailyOpsDuePolicy {
            policy_key: "test-policy".to_string(),
            due_window_minutes: 60,
            escalation_window_minutes: 120,
            source: "test".to_string(),
        }
    }

    #[test]
    fn critical_alert_scores_higher_than_low_ticket() {
        let (alert_score, _, _) = score_alert_item("critical", "open", 30);
        let (ticket_score, _, _) = score_ticket_item("low", "open", 30);
        assert!(alert_score > ticket_score);
    }

    #[test]
    fn queue_sort_is_deterministic_by_score_then_time_then_key() {
        let mut items = vec![
            test_item("b", 700, 5),
            test_item("a", 700, 5),
            test_item("c", 900, 30),
            test_item("d", 700, 1),
        ];
        sort_daily_queue_items(&mut items);

        assert_eq!(items[0].queue_key, "c");
        assert_eq!(items[1].queue_key, "d");
        assert_eq!(items[2].queue_key, "a");
        assert_eq!(items[3].queue_key, "b");
    }

    #[test]
    fn next_best_action_sort_is_deterministic() {
        let now = Utc::now();
        let mut items = vec![
            NextBestActionItem {
                suggestion_key: "handover:item-2".to_string(),
                domain: "handover".to_string(),
                priority_score: 820,
                risk_level: "high".to_string(),
                reason: "r".to_string(),
                source_signal: "s".to_string(),
                observed_at: now,
                entity: serde_json::json!({}),
                action: DailyCockpitAction {
                    key: "noop".to_string(),
                    label: "noop".to_string(),
                    href: None,
                    api_path: None,
                    method: None,
                    body: None,
                    requires_write: false,
                },
            },
            NextBestActionItem {
                suggestion_key: "incident:11".to_string(),
                domain: "incident".to_string(),
                priority_score: 920,
                risk_level: "critical".to_string(),
                reason: "r".to_string(),
                source_signal: "s".to_string(),
                observed_at: now,
                entity: serde_json::json!({}),
                action: DailyCockpitAction {
                    key: "noop".to_string(),
                    label: "noop".to_string(),
                    href: None,
                    api_path: None,
                    method: None,
                    body: None,
                    requires_write: false,
                },
            },
            NextBestActionItem {
                suggestion_key: "incident:10".to_string(),
                domain: "incident".to_string(),
                priority_score: 920,
                risk_level: "critical".to_string(),
                reason: "r".to_string(),
                source_signal: "s".to_string(),
                observed_at: now,
                entity: serde_json::json!({}),
                action: DailyCockpitAction {
                    key: "noop".to_string(),
                    label: "noop".to_string(),
                    href: None,
                    api_path: None,
                    method: None,
                    body: None,
                    requires_write: false,
                },
            },
        ];

        sort_next_best_actions(&mut items);
        let keys = items
            .into_iter()
            .map(|item| item.suggestion_key)
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "incident:10".to_string(),
                "incident:11".to_string(),
                "handover:item-2".to_string()
            ]
        );
    }

    #[test]
    fn go_live_summary_counts_statuses_and_derives_overall() {
        let summary = summarize_go_live_readiness(&[
            GoLiveReadinessDomainItem {
                domain_key: "authentication".to_string(),
                name: "Authentication".to_string(),
                status: "ready".to_string(),
                summary: "ok".to_string(),
                reason: "ok".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            GoLiveReadinessDomainItem {
                domain_key: "monitoring_sources".to_string(),
                name: "Monitoring".to_string(),
                status: "warning".to_string(),
                summary: "warn".to_string(),
                reason: "warn".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            GoLiveReadinessDomainItem {
                domain_key: "backup_restore".to_string(),
                name: "Backup".to_string(),
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
        assert_eq!(derive_go_live_overall_status(&summary), "blocking");
    }

    #[test]
    fn selects_first_blocking_or_warning_go_live_domain() {
        let domains = vec![
            GoLiveReadinessDomainItem {
                domain_key: "authentication".to_string(),
                name: "Authentication".to_string(),
                status: "ready".to_string(),
                summary: "ok".to_string(),
                reason: "ok".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            GoLiveReadinessDomainItem {
                domain_key: "monitoring_sources".to_string(),
                name: "Monitoring".to_string(),
                status: "warning".to_string(),
                summary: "warn".to_string(),
                reason: "warn".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
            GoLiveReadinessDomainItem {
                domain_key: "backup_restore".to_string(),
                name: "Backup".to_string(),
                status: "blocking".to_string(),
                summary: "block".to_string(),
                reason: "block".to_string(),
                recommended_action: None,
                evidence: json!({}),
            },
        ];

        assert_eq!(
            select_next_go_live_domain(&domains),
            Some("backup_restore".to_string())
        );
        assert_eq!(
            select_next_go_live_domain(&domains[..2]),
            Some("monitoring_sources".to_string())
        );
    }

    #[test]
    fn checklist_response_marks_overdue_and_summarizes_status() {
        let checklist_date = Utc::now().date_naive() - Duration::days(1);
        let weekday = checklist_date.weekday().number_from_monday() as i16;
        let weekly_next = if weekday == 7 { 1 } else { weekday + 1 };
        let now = Utc::now();

        let templates = vec![
            OpsChecklistTemplateRow {
                id: 1,
                template_key: "daily-alert-queue-review".to_string(),
                title: "Daily Alert Queue Review".to_string(),
                description: None,
                frequency: "daily".to_string(),
                due_weekday: None,
                guidance: Some("daily guidance".to_string()),
            },
            OpsChecklistTemplateRow {
                id: 2,
                template_key: "daily-monitoring-sync-backlog".to_string(),
                title: "Daily Monitoring Sync Backlog Sweep".to_string(),
                description: None,
                frequency: "daily".to_string(),
                due_weekday: None,
                guidance: Some("backlog guidance".to_string()),
            },
            OpsChecklistTemplateRow {
                id: 3,
                template_key: "weekly-break-glass-review".to_string(),
                title: "Weekly Break-Glass Review".to_string(),
                description: None,
                frequency: "weekly".to_string(),
                due_weekday: Some(weekday),
                guidance: Some("weekly guidance".to_string()),
            },
            OpsChecklistTemplateRow {
                id: 4,
                template_key: "weekly-capacity-review".to_string(),
                title: "Weekly Capacity Review".to_string(),
                description: None,
                frequency: "weekly".to_string(),
                due_weekday: Some(weekly_next),
                guidance: Some("capacity guidance".to_string()),
            },
        ];
        let entries = vec![
            OpsChecklistEntryRow {
                template_id: 1,
                status: "completed".to_string(),
                exception_note: None,
                completed_at: Some(now),
                updated_at: now,
            },
            OpsChecklistEntryRow {
                template_id: 2,
                status: "skipped".to_string(),
                exception_note: Some("deferred by operator".to_string()),
                completed_at: None,
                updated_at: now,
            },
        ];

        let response = build_ops_checklist_response(
            checklist_date,
            "operator".to_string(),
            "".to_string(),
            "".to_string(),
            templates,
            entries,
        );

        assert_eq!(response.summary.total, 3);
        assert_eq!(response.summary.completed, 1);
        assert_eq!(response.summary.skipped, 1);
        assert_eq!(response.summary.pending, 1);
        assert_eq!(response.summary.overdue, 1);
        assert_eq!(response.items.len(), 3);

        let pending = response
            .items
            .iter()
            .find(|item| item.template_key == "weekly-break-glass-review")
            .expect("weekly checklist due item should exist");
        assert_eq!(pending.status, "pending");
        assert!(pending.overdue);

        let skipped = response
            .items
            .iter()
            .find(|item| item.template_key == "daily-monitoring-sync-backlog")
            .expect("skipped checklist item should exist");
        assert_eq!(skipped.status, "skipped");
        assert_eq!(
            skipped.exception_note.as_deref(),
            Some("deferred by operator")
        );
    }

    #[test]
    fn optional_note_is_trimmed_and_has_length_limit() {
        assert_eq!(
            normalize_optional_note(None).expect("none should pass"),
            None
        );
        assert_eq!(
            normalize_optional_note(Some("  acknowledged  ".to_string()))
                .expect("trim should pass"),
            Some("acknowledged".to_string())
        );
        assert_eq!(
            normalize_optional_note(Some("   ".to_string())).expect("blank should pass"),
            None
        );

        let too_long = "x".repeat(MAX_CHECKLIST_NOTE_LEN + 1);
        assert!(normalize_optional_note(Some(too_long)).is_err());
    }

    #[test]
    fn daily_ops_status_prefers_completed_and_future_defer() {
        let now = Utc::now();
        let completed_state = DailyOpsFollowUpStateRow {
            task_key: "ticket:1".to_string(),
            item_type: "ticket".to_string(),
            follow_up_state: DAILY_OPS_FOLLOW_UP_STATE_COMPLETED.to_string(),
            note: Some("done".to_string()),
            defer_until: None,
            actor: "operator".to_string(),
            updated_at: now,
        };
        let deferred_state = DailyOpsFollowUpStateRow {
            task_key: "ticket:2".to_string(),
            item_type: "ticket".to_string(),
            follow_up_state: "deferred".to_string(),
            note: Some("later".to_string()),
            defer_until: Some(now + Duration::hours(4)),
            actor: "operator".to_string(),
            updated_at: now,
        };

        assert_eq!(
            derive_daily_ops_status("overdue", Some(&completed_state), now),
            "completed"
        );
        assert_eq!(
            derive_daily_ops_status("due_today", Some(&deferred_state), now),
            "deferred"
        );
        assert_eq!(derive_daily_ops_status("blocked", None, now), "blocked");
    }

    #[test]
    fn daily_ops_summary_counts_statuses_and_acknowledgements() {
        let now = Utc::now();
        let summary = summarize_daily_ops_items(&[
            DailyOpsFollowUpItem {
                task_key: "alert:1".to_string(),
                item_type: "alert".to_string(),
                domain: "alert".to_string(),
                status: "overdue".to_string(),
                follow_up_state: "new".to_string(),
                priority: "critical".to_string(),
                owner: test_daily_ops_owner(),
                summary: "a".to_string(),
                reason: "a".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: None,
                completed_at: None,
                deferred_until: None,
                evidence: json!({}),
            },
            DailyOpsFollowUpItem {
                task_key: "ticket:1".to_string(),
                item_type: "ticket".to_string(),
                domain: "ticket".to_string(),
                status: "completed".to_string(),
                follow_up_state: DAILY_OPS_FOLLOW_UP_STATE_COMPLETED.to_string(),
                priority: "medium".to_string(),
                owner: test_daily_ops_owner(),
                summary: "b".to_string(),
                reason: "b".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: None,
                completed_at: Some(now),
                deferred_until: None,
                evidence: json!({}),
            },
            DailyOpsFollowUpItem {
                task_key: "go-live:1".to_string(),
                item_type: "go_live".to_string(),
                domain: "go_live".to_string(),
                status: "due_today".to_string(),
                follow_up_state: DAILY_OPS_FOLLOW_UP_STATE_ACKNOWLEDGED.to_string(),
                priority: "high".to_string(),
                owner: test_daily_ops_owner(),
                summary: "c".to_string(),
                reason: "c".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: Some(now),
                completed_at: None,
                deferred_until: None,
                evidence: json!({}),
            },
        ]);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.overdue, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.due_today, 1);
        assert_eq!(summary.acknowledged, 1);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.high, 1);
        assert_eq!(summary.medium, 1);
    }

    #[test]
    fn daily_ops_sort_prioritizes_blocked_and_overdue_first() {
        let now = Utc::now();
        let mut items = vec![
            DailyOpsFollowUpItem {
                task_key: "a".to_string(),
                item_type: "alert".to_string(),
                domain: "alert".to_string(),
                status: "due_today".to_string(),
                follow_up_state: "new".to_string(),
                priority: "critical".to_string(),
                owner: test_daily_ops_owner(),
                summary: "a".to_string(),
                reason: "a".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now + Duration::hours(1)),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: None,
                completed_at: None,
                deferred_until: None,
                evidence: json!({}),
            },
            DailyOpsFollowUpItem {
                task_key: "b".to_string(),
                item_type: "go_live".to_string(),
                domain: "go_live".to_string(),
                status: "blocked".to_string(),
                follow_up_state: "new".to_string(),
                priority: "high".to_string(),
                owner: test_daily_ops_owner(),
                summary: "b".to_string(),
                reason: "b".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: None,
                completed_at: None,
                deferred_until: None,
                evidence: json!({}),
            },
            DailyOpsFollowUpItem {
                task_key: "c".to_string(),
                item_type: "ticket".to_string(),
                domain: "ticket".to_string(),
                status: "overdue".to_string(),
                follow_up_state: "new".to_string(),
                priority: "medium".to_string(),
                owner: test_daily_ops_owner(),
                summary: "c".to_string(),
                reason: "c".to_string(),
                recommended_action: None,
                available_actions: vec![],
                due_at: Some(now - Duration::hours(1)),
                escalate_at: None,
                due_policy: test_daily_ops_due_policy(),
                observed_at: now,
                acknowledged_at: None,
                completed_at: None,
                deferred_until: None,
                evidence: json!({}),
            },
        ];

        sort_daily_ops_items(&mut items);
        let keys = items
            .into_iter()
            .map(|item| item.task_key)
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec!["b".to_string(), "c".to_string(), "a".to_string()]
        );
    }

    #[test]
    fn validates_daily_ops_action() {
        assert_eq!(
            normalize_daily_ops_action(" complete ".to_string()).expect("valid"),
            "complete"
        );
        assert!(normalize_daily_ops_action("unknown".to_string()).is_err());
    }

    #[test]
    fn parses_optional_date_with_expected_format() {
        assert_eq!(
            parse_optional_date(Some("2026-03-06".to_string())).expect("date should parse"),
            NaiveDate::from_ymd_opt(2026, 3, 6).expect("date is valid")
        );
        assert!(parse_optional_date(Some("2026/03/06".to_string())).is_err());
    }
}
