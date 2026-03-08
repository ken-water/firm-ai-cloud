use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

const DEFAULT_RANGE_DAYS: u32 = 14;
const MAX_RANGE_DAYS: u32 = 31;
const MAX_OPERATION_KIND_LEN: usize = 128;
const MAX_OWNER_LEN: usize = 128;
const MAX_SCOPE_LEN: usize = 128;
const MAX_NOTE_LEN: usize = 1024;
const MAX_RESERVATION_LIMIT: u32 = 200;
const DEFAULT_RESERVATION_LIMIT: u32 = 80;
const MAX_SLOT_RECOMMENDATION_LIMIT: u32 = 20;
const DEFAULT_SLOT_RECOMMENDATION_LIMIT: u32 = 5;
const MAX_SLOT_DURATION_MINUTES: u32 = 8 * 60;
const MIN_SLOT_DURATION_MINUTES: u32 = 15;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/change-calendar", get(get_change_calendar))
        .route(
            "/cockpit/change-calendar/conflicts",
            post(check_change_calendar_conflicts),
        )
        .route(
            "/cockpit/change-calendar/reservations",
            get(list_change_calendar_reservations).post(create_change_calendar_reservation),
        )
        .route(
            "/cockpit/change-calendar/slot-recommendations",
            get(get_change_calendar_slot_recommendations),
        )
}

#[derive(Debug, Deserialize, Default)]
struct ChangeCalendarQuery {
    start_date: Option<String>,
    end_date: Option<String>,
    days: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChangeCalendarConflictRequest {
    start_at: String,
    end_at: String,
    operation_kind: Option<String>,
    risk_level: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ChangeCalendarReservationListQuery {
    start_date: Option<String>,
    end_date: Option<String>,
    days: Option<u32>,
    status: Option<String>,
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChangeCalendarReservationCreateRequest {
    start_at: String,
    end_at: String,
    operation_kind: String,
    risk_level: String,
    owner: String,
    site: Option<String>,
    department: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ChangeCalendarSlotRecommendationQuery {
    operation_kind: Option<String>,
    risk_level: Option<String>,
    duration_minutes: Option<u32>,
    from_time: Option<String>,
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
struct ChangeCalendarEvent {
    event_key: String,
    event_type: String,
    severity: String,
    title: String,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    source_type: String,
    source_id: String,
    details: String,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarResponse {
    generated_at: DateTime<Utc>,
    range: ChangeCalendarRange,
    total: usize,
    items: Vec<ChangeCalendarEvent>,
}

#[derive(Debug, Serialize, Clone, FromRow)]
struct ChangeCalendarReservationRecord {
    id: i64,
    operation_kind: String,
    risk_level: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
    owner: String,
    note: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarReservationListResponse {
    generated_at: DateTime<Utc>,
    range: ChangeCalendarRange,
    total: usize,
    items: Vec<ChangeCalendarReservationRecord>,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarReservationCreateResponse {
    generated_at: DateTime<Utc>,
    reservation: ChangeCalendarReservationRecord,
    decision_reason: String,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarSlotRecommendationResponse {
    generated_at: DateTime<Utc>,
    operation_kind: String,
    risk_level: String,
    duration_minutes: u32,
    scope: ChangeCalendarRecommendationScope,
    pending_risky_workload: PendingRiskyWorkloadSummary,
    total: usize,
    items: Vec<ChangeCalendarSlotRecommendationItem>,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarRecommendationScope {
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize, Default)]
struct PendingRiskyWorkloadSummary {
    unresolved_incidents: i64,
    high_priority_tickets: i64,
    pending_approvals: i64,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarSlotRecommendationItem {
    rank: usize,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    score: i32,
    rationale: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarRange {
    start_date: String,
    end_date: String,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarConflictResponse {
    generated_at: DateTime<Utc>,
    slot: ChangeCalendarSlot,
    has_conflict: bool,
    decision_reason: String,
    conflicts: Vec<CalendarConflictItem>,
    recommended_slot: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ChangeCalendarSlot {
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    operation_kind: String,
    risk_level: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CalendarConflictItem {
    pub code: String,
    pub title: String,
    pub detail: String,
    pub severity: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ChangeCalendarConflictEvaluation {
    pub has_conflict: bool,
    pub decision_reason: String,
    pub conflicts: Vec<CalendarConflictItem>,
    pub recommended_slot: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct PlaybookPolicyRow {
    timezone_name: String,
    maintenance_windows: Value,
    change_freeze_enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct MaintenanceWindow {
    day_of_week: u8,
    start: String,
    end: String,
    label: Option<String>,
}

#[derive(Debug, FromRow)]
struct PendingWorkflowApprovalRow {
    id: i64,
    title: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PendingPlaybookApprovalRow {
    id: i64,
    playbook_key: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct BackupScheduleRow {
    id: i64,
    policy_key: String,
    next_backup_at: Option<DateTime<Utc>>,
    next_drill_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct ReservationEventRow {
    id: i64,
    operation_kind: String,
    risk_level: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    site: Option<String>,
    department: Option<String>,
    owner: String,
    note: Option<String>,
}

async fn get_change_calendar(
    State(state): State<AppState>,
    Query(query): Query<ChangeCalendarQuery>,
) -> AppResult<Json<ChangeCalendarResponse>> {
    let (start_date, end_date) =
        parse_change_calendar_range(query.start_date, query.end_date, query.days)?;

    let items = load_change_calendar_events(&state, start_date, end_date).await?;

    Ok(Json(ChangeCalendarResponse {
        generated_at: Utc::now(),
        range: ChangeCalendarRange {
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
        },
        total: items.len(),
        items,
    }))
}

async fn check_change_calendar_conflicts(
    State(state): State<AppState>,
    Json(payload): Json<ChangeCalendarConflictRequest>,
) -> AppResult<Json<ChangeCalendarConflictResponse>> {
    let start_at = DateTime::parse_from_rfc3339(payload.start_at.trim())
        .map_err(|_| AppError::Validation("start_at must use RFC3339 format".to_string()))?
        .with_timezone(&Utc);
    let end_at = DateTime::parse_from_rfc3339(payload.end_at.trim())
        .map_err(|_| AppError::Validation("end_at must use RFC3339 format".to_string()))?
        .with_timezone(&Utc);
    if end_at <= start_at {
        return Err(AppError::Validation(
            "end_at must be later than start_at".to_string(),
        ));
    }

    let operation_kind = payload
        .operation_kind
        .unwrap_or_else(|| "playbook.execute".to_string())
        .trim()
        .to_string();
    let risk_level = payload
        .risk_level
        .unwrap_or_else(|| "medium".to_string())
        .trim()
        .to_ascii_lowercase();
    let evaluation = evaluate_change_calendar_conflicts(
        &state.db,
        start_at,
        end_at,
        operation_kind.as_str(),
        risk_level.as_str(),
    )
    .await?;

    Ok(Json(ChangeCalendarConflictResponse {
        generated_at: Utc::now(),
        slot: ChangeCalendarSlot {
            start_at,
            end_at,
            operation_kind,
            risk_level,
        },
        has_conflict: evaluation.has_conflict,
        decision_reason: evaluation.decision_reason,
        conflicts: evaluation.conflicts,
        recommended_slot: evaluation.recommended_slot,
    }))
}

async fn list_change_calendar_reservations(
    State(state): State<AppState>,
    Query(query): Query<ChangeCalendarReservationListQuery>,
) -> AppResult<Json<ChangeCalendarReservationListResponse>> {
    let (start_date, end_date) =
        parse_change_calendar_range(query.start_date, query.end_date, query.days)?;
    let status = normalize_optional_status(query.status)?;
    let site = normalize_optional_scope(query.site);
    let department = normalize_optional_scope(query.department);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_RESERVATION_LIMIT)
        .clamp(1, MAX_RESERVATION_LIMIT) as i64;

    let range_start = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).expect("midnight"));
    let range_end = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).expect("end-of-day"));

    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, operation_kind, risk_level, start_at, end_at, site, department, owner, note, status, created_at, updated_at
         FROM ops_change_calendar_reservations
         WHERE start_at <= ",
    );
    builder
        .push_bind(range_end)
        .push(" AND end_at >= ")
        .push_bind(range_start);
    if let Some(status) = status.as_deref() {
        builder.push(" AND status = ").push_bind(status);
    }
    if let Some(site) = site.as_deref() {
        builder.push(" AND site = ").push_bind(site);
    }
    if let Some(department) = department.as_deref() {
        builder.push(" AND department = ").push_bind(department);
    }
    builder
        .push(" ORDER BY start_at ASC, id ASC LIMIT ")
        .push_bind(limit);

    let items: Vec<ChangeCalendarReservationRecord> =
        builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ChangeCalendarReservationListResponse {
        generated_at: Utc::now(),
        range: ChangeCalendarRange {
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
        },
        total: items.len(),
        items,
    }))
}

async fn create_change_calendar_reservation(
    State(state): State<AppState>,
    Json(payload): Json<ChangeCalendarReservationCreateRequest>,
) -> AppResult<Json<ChangeCalendarReservationCreateResponse>> {
    let start_at = DateTime::parse_from_rfc3339(payload.start_at.trim())
        .map_err(|_| AppError::Validation("start_at must use RFC3339 format".to_string()))?
        .with_timezone(&Utc);
    let end_at = DateTime::parse_from_rfc3339(payload.end_at.trim())
        .map_err(|_| AppError::Validation("end_at must use RFC3339 format".to_string()))?
        .with_timezone(&Utc);
    if end_at <= start_at {
        return Err(AppError::Validation(
            "end_at must be later than start_at".to_string(),
        ));
    }

    let operation_kind = required_trimmed(
        "operation_kind",
        payload.operation_kind,
        MAX_OPERATION_KIND_LEN,
    )?;
    let risk_level = normalize_risk_level(payload.risk_level)?;
    let owner = required_trimmed("owner", payload.owner, MAX_OWNER_LEN)?;
    let site = trim_optional(payload.site, MAX_SCOPE_LEN);
    let department = trim_optional(payload.department, MAX_SCOPE_LEN);
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let overlap_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_change_calendar_reservations
         WHERE status = 'reserved'
           AND start_at < $2
           AND end_at > $1
           AND ($3::text IS NULL OR site = $3)
           AND ($4::text IS NULL OR department = $4)",
    )
    .bind(start_at)
    .bind(end_at)
    .bind(site.as_deref())
    .bind(department.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    if overlap_count > 0 {
        return Err(AppError::Validation(format!(
            "reservation conflicts with {overlap_count} existing reserved slot(s) in selected scope"
        )));
    }

    let evaluation = evaluate_change_calendar_conflicts(
        &state.db,
        start_at,
        end_at,
        operation_kind.as_str(),
        risk_level.as_str(),
    )
    .await?;
    let blocking_conflicts = evaluation
        .conflicts
        .iter()
        .filter(|item| item.code != "approval_backlog")
        .collect::<Vec<_>>();
    if !blocking_conflicts.is_empty() {
        let details = blocking_conflicts
            .iter()
            .map(|item| format!("[{}] {}: {}", item.severity, item.title, item.detail))
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(AppError::Validation(format!(
            "reservation rejected: {} {}",
            evaluation.decision_reason, details
        )));
    }

    let reservation: ChangeCalendarReservationRecord = sqlx::query_as(
        "INSERT INTO ops_change_calendar_reservations (
            operation_kind, risk_level, start_at, end_at, site, department, owner, note, status
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'reserved')
         RETURNING id, operation_kind, risk_level, start_at, end_at, site, department,
                   owner, note, status, created_at, updated_at",
    )
    .bind(operation_kind)
    .bind(risk_level)
    .bind(start_at)
    .bind(end_at)
    .bind(site)
    .bind(department)
    .bind(owner)
    .bind(note)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ChangeCalendarReservationCreateResponse {
        generated_at: Utc::now(),
        reservation,
        decision_reason: evaluation.decision_reason,
    }))
}

async fn get_change_calendar_slot_recommendations(
    State(state): State<AppState>,
    Query(query): Query<ChangeCalendarSlotRecommendationQuery>,
) -> AppResult<Json<ChangeCalendarSlotRecommendationResponse>> {
    let operation_kind = trim_optional(query.operation_kind, MAX_OPERATION_KIND_LEN)
        .unwrap_or_else(|| "playbook.execute".to_string());
    let risk_level =
        normalize_risk_level(query.risk_level.unwrap_or_else(|| "medium".to_string()))?;
    let duration_minutes = query
        .duration_minutes
        .unwrap_or(60)
        .clamp(MIN_SLOT_DURATION_MINUTES, MAX_SLOT_DURATION_MINUTES);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_SLOT_RECOMMENDATION_LIMIT)
        .clamp(1, MAX_SLOT_RECOMMENDATION_LIMIT) as usize;
    let from_time = parse_optional_rfc3339(query.from_time)?;
    let site = normalize_optional_scope(query.site);
    let department = normalize_optional_scope(query.department);
    let policy = load_playbook_policy(&state.db).await?;
    let windows = parse_maintenance_windows(policy.maintenance_windows);
    let timezone = parse_timezone(policy.timezone_name.as_str());
    let workload = load_pending_risky_workload(&state.db).await;

    let candidate_starts =
        collect_candidate_slot_starts(&windows, timezone, from_time, duration_minutes as i64);
    let mut items = Vec::new();
    for start_at in candidate_starts.into_iter().take(120) {
        let end_at = start_at + Duration::minutes(duration_minutes as i64);
        let overlap_count = count_reservation_overlaps(
            &state.db,
            start_at,
            end_at,
            site.as_deref(),
            department.as_deref(),
        )
        .await
        .unwrap_or(0);
        if overlap_count > 0 {
            continue;
        }

        let evaluation = evaluate_change_calendar_conflicts(
            &state.db,
            start_at,
            end_at,
            operation_kind.as_str(),
            risk_level.as_str(),
        )
        .await?;
        let has_blocking_conflict = evaluation
            .conflicts
            .iter()
            .any(|item| item.code != "approval_backlog");
        if has_blocking_conflict {
            continue;
        }

        let age_minutes = (start_at - from_time).num_minutes().max(0) as i32;
        let workload_penalty = ((workload.unresolved_incidents
            + workload.high_priority_tickets
            + workload.pending_approvals)
            .min(100) as i32)
            * 3;
        let score = 2000 - age_minutes - workload_penalty;
        items.push(ChangeCalendarSlotRecommendationItem {
            rank: 0,
            start_at,
            end_at,
            score,
            rationale: vec![
                "slot is inside maintenance policy windows".to_string(),
                format!(
                    "pending workload: incidents={}, high_priority_tickets={}, approvals={}",
                    workload.unresolved_incidents,
                    workload.high_priority_tickets,
                    workload.pending_approvals
                ),
                format!("calendar decision: {}", evaluation.decision_reason),
            ],
        });
    }

    items.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.start_at.cmp(&right.start_at))
            .then_with(|| left.end_at.cmp(&right.end_at))
    });
    if items.len() > limit {
        items.truncate(limit);
    }
    for (idx, item) in items.iter_mut().enumerate() {
        item.rank = idx + 1;
    }

    Ok(Json(ChangeCalendarSlotRecommendationResponse {
        generated_at: Utc::now(),
        operation_kind,
        risk_level,
        duration_minutes,
        scope: ChangeCalendarRecommendationScope { site, department },
        pending_risky_workload: workload,
        total: items.len(),
        items,
    }))
}

pub async fn evaluate_change_calendar_conflicts(
    db: &sqlx::PgPool,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    operation_kind: &str,
    risk_level: &str,
) -> AppResult<ChangeCalendarConflictEvaluation> {
    if end_at <= start_at {
        return Err(AppError::Validation(
            "end_at must be later than start_at".to_string(),
        ));
    }

    let operation_kind = if operation_kind.trim().is_empty() {
        "playbook.execute"
    } else {
        operation_kind.trim()
    };
    let risk_level = if risk_level.trim().is_empty() {
        "medium".to_string()
    } else {
        risk_level.trim().to_ascii_lowercase()
    };

    let policy = load_playbook_policy(db).await?;
    let windows = parse_maintenance_windows(policy.maintenance_windows);
    let timezone = parse_timezone(policy.timezone_name.as_str());

    let mut conflicts = Vec::new();
    if policy.change_freeze_enabled {
        conflicts.push(CalendarConflictItem {
            code: "change_freeze".to_string(),
            title: "Change freeze enabled".to_string(),
            detail: "Global change freeze is enabled in playbook execution policy.".to_string(),
            severity: "critical".to_string(),
            source: "workflow_playbook_execution_policies".to_string(),
        });
    }

    let requires_window = matches!(risk_level.as_str(), "high" | "critical");
    if requires_window && !slot_is_in_maintenance_window(&windows, timezone, start_at, end_at) {
        conflicts.push(CalendarConflictItem {
            code: "outside_maintenance_window".to_string(),
            title: "Outside maintenance window".to_string(),
            detail:
                "Proposed slot is outside configured maintenance windows for high-risk operation."
                    .to_string(),
            severity: "high".to_string(),
            source: "workflow_playbook_execution_policies".to_string(),
        });
    }

    let pending_workflow: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_requests
         WHERE status = 'pending_approval'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let pending_playbook: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at > NOW()",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let pending_total = pending_workflow + pending_playbook;
    if pending_total > 0 {
        conflicts.push(CalendarConflictItem {
            code: "approval_backlog".to_string(),
            title: "Pending approvals backlog".to_string(),
            detail: format!(
                "There are {} pending approvals (workflow={}, playbook={}).",
                pending_total, pending_workflow, pending_playbook
            ),
            severity: if pending_total >= 5 { "high" } else { "medium" }.to_string(),
            source: "workflow_requests/workflow_playbook_approval_requests".to_string(),
        });
    }

    let has_conflict = !conflicts.is_empty();
    let decision_reason = if has_conflict {
        let conflict_codes = conflicts
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "Conflict detected for {} [{}]: {}.",
            operation_kind, risk_level, conflict_codes
        )
    } else {
        format!(
            "No conflict detected for {} [{}] in requested slot.",
            operation_kind, risk_level
        )
    };

    Ok(ChangeCalendarConflictEvaluation {
        has_conflict,
        decision_reason,
        conflicts,
        recommended_slot: find_next_maintenance_slot(&windows, timezone, end_at),
    })
}

async fn load_change_calendar_events(
    state: &AppState,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<Vec<ChangeCalendarEvent>> {
    let policy = load_playbook_policy(&state.db).await?;
    let windows = parse_maintenance_windows(policy.maintenance_windows);
    let timezone = parse_timezone(policy.timezone_name.as_str());

    let mut items: Vec<ChangeCalendarEvent> = Vec::new();
    let mut cursor = start_date;
    while cursor <= end_date {
        for (index, window) in windows.iter().enumerate() {
            if window.day_of_week != cursor.weekday().number_from_monday() as u8 {
                continue;
            }
            let Some((starts_at, ends_at)) = build_window_bounds(cursor, window, timezone) else {
                continue;
            };
            items.push(ChangeCalendarEvent {
                event_key: format!("maintenance:{}:{}:{}", cursor, window.day_of_week, index),
                event_type: "maintenance_window".to_string(),
                severity: "medium".to_string(),
                title: window
                    .label
                    .clone()
                    .unwrap_or_else(|| "Maintenance window".to_string()),
                starts_at,
                ends_at,
                source_type: "playbook_policy".to_string(),
                source_id: "global".to_string(),
                details: format!("{} {}-{}", cursor, window.start, window.end),
            });
        }

        if policy.change_freeze_enabled {
            let start_dt = Utc.from_utc_datetime(
                &cursor
                    .and_hms_opt(0, 0, 0)
                    .expect("valid midnight for freeze"),
            );
            let end_dt = Utc.from_utc_datetime(
                &cursor
                    .and_hms_opt(23, 59, 59)
                    .expect("valid end-of-day for freeze"),
            );
            items.push(ChangeCalendarEvent {
                event_key: format!("freeze:{cursor}"),
                event_type: "change_freeze".to_string(),
                severity: "critical".to_string(),
                title: "Change freeze enabled".to_string(),
                starts_at: start_dt,
                ends_at: end_dt,
                source_type: "playbook_policy".to_string(),
                source_id: "global".to_string(),
                details: "Global change freeze applies for this day.".to_string(),
            });
        }

        cursor += Duration::days(1);
    }

    let range_start = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0).expect("midnight"));
    let range_end = Utc.from_utc_datetime(&end_date.and_hms_opt(23, 59, 59).expect("end-of-day"));

    let workflow_approvals: Vec<PendingWorkflowApprovalRow> = sqlx::query_as(
        "SELECT id, title, created_at
         FROM workflow_requests
         WHERE status = 'pending_approval'
           AND created_at <= $2
         ORDER BY created_at ASC, id ASC
         LIMIT 120",
    )
    .bind(range_start)
    .bind(range_end)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for row in workflow_approvals {
        let starts_at = row.created_at;
        let ends_at = row.created_at + Duration::hours(2);
        if ends_at < range_start || starts_at > range_end {
            continue;
        }
        items.push(ChangeCalendarEvent {
            event_key: format!("workflow_approval:{}", row.id),
            event_type: "pending_workflow_approval".to_string(),
            severity: "medium".to_string(),
            title: row.title,
            starts_at,
            ends_at,
            source_type: "workflow_request".to_string(),
            source_id: row.id.to_string(),
            details: "Workflow request pending approval.".to_string(),
        });
    }

    let playbook_approvals: Vec<PendingPlaybookApprovalRow> = sqlx::query_as(
        "SELECT id, playbook_key, created_at, expires_at
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at >= $1
         ORDER BY created_at ASC, id ASC
         LIMIT 120",
    )
    .bind(range_start)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for row in playbook_approvals {
        if row.created_at > range_end {
            continue;
        }
        items.push(ChangeCalendarEvent {
            event_key: format!("playbook_approval:{}", row.id),
            event_type: "pending_playbook_approval".to_string(),
            severity: "high".to_string(),
            title: format!("{} pending approval", row.playbook_key),
            starts_at: row.created_at,
            ends_at: row.expires_at,
            source_type: "playbook_approval".to_string(),
            source_id: row.id.to_string(),
            details: "High-risk playbook approval pending.".to_string(),
        });
    }

    let backup_schedules: Vec<BackupScheduleRow> = sqlx::query_as(
        "SELECT id, policy_key, next_backup_at, next_drill_at
         FROM ops_backup_policies
         ORDER BY id ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for row in backup_schedules {
        if let Some(next_backup_at) = row.next_backup_at {
            if next_backup_at >= range_start && next_backup_at <= range_end {
                items.push(ChangeCalendarEvent {
                    event_key: format!("backup_schedule:{}:backup", row.id),
                    event_type: "scheduled_backup".to_string(),
                    severity: "low".to_string(),
                    title: format!("{} backup schedule", row.policy_key),
                    starts_at: next_backup_at,
                    ends_at: next_backup_at + Duration::minutes(30),
                    source_type: "backup_policy".to_string(),
                    source_id: row.id.to_string(),
                    details: "Scheduled backup run.".to_string(),
                });
            }
        }
        if let Some(next_drill_at) = row.next_drill_at {
            if next_drill_at >= range_start && next_drill_at <= range_end {
                items.push(ChangeCalendarEvent {
                    event_key: format!("backup_schedule:{}:drill", row.id),
                    event_type: "scheduled_drill".to_string(),
                    severity: "medium".to_string(),
                    title: format!("{} drill schedule", row.policy_key),
                    starts_at: next_drill_at,
                    ends_at: next_drill_at + Duration::minutes(45),
                    source_type: "backup_policy".to_string(),
                    source_id: row.id.to_string(),
                    details: "Scheduled drill run.".to_string(),
                });
            }
        }
    }

    let reservation_rows: Vec<ReservationEventRow> = sqlx::query_as(
        "SELECT id, operation_kind, risk_level, start_at, end_at, site, department, owner, note
         FROM ops_change_calendar_reservations
         WHERE status = 'reserved'
           AND start_at <= $2
           AND end_at >= $1
         ORDER BY start_at ASC, id ASC
         LIMIT 240",
    )
    .bind(range_start)
    .bind(range_end)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for row in reservation_rows {
        items.push(ChangeCalendarEvent {
            event_key: format!("reservation:{}", row.id),
            event_type: "operation_reservation".to_string(),
            severity: row.risk_level.clone(),
            title: format!("{} reserved by {}", row.operation_kind, row.owner),
            starts_at: row.start_at,
            ends_at: row.end_at,
            source_type: "reservation".to_string(),
            source_id: row.id.to_string(),
            details: format!(
                "risk={} scope={}/{} note={}",
                row.risk_level,
                row.site.as_deref().unwrap_or("-"),
                row.department.as_deref().unwrap_or("-"),
                row.note.as_deref().unwrap_or("-")
            ),
        });
    }

    items.sort_by(|left, right| {
        left.starts_at
            .cmp(&right.starts_at)
            .then_with(|| left.event_type.cmp(&right.event_type))
            .then_with(|| left.event_key.cmp(&right.event_key))
    });

    Ok(items)
}

async fn load_playbook_policy(db: &sqlx::PgPool) -> AppResult<PlaybookPolicyRow> {
    let row: Option<PlaybookPolicyRow> = sqlx::query_as(
        "SELECT timezone_name, maintenance_windows, change_freeze_enabled
         FROM workflow_playbook_execution_policies
         WHERE policy_key = 'global'",
    )
    .fetch_optional(db)
    .await?;

    Ok(row.unwrap_or(PlaybookPolicyRow {
        timezone_name: "UTC".to_string(),
        maintenance_windows: Value::Array(vec![]),
        change_freeze_enabled: false,
    }))
}

async fn load_pending_risky_workload(db: &sqlx::PgPool) -> PendingRiskyWorkloadSummary {
    let unresolved_incidents: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_incident_commands c
         LEFT JOIN unified_alerts a ON a.id = c.alert_id
         WHERE c.command_status IN ('triage', 'in_progress', 'blocked')
           AND COALESCE(a.severity, 'warning') IN ('critical', 'warning')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let high_priority_tickets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM tickets
         WHERE status IN ('open', 'in_progress')
           AND priority IN ('critical', 'high')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let pending_approvals_workflow: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_requests
         WHERE status = 'pending_approval'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let pending_approvals_playbook: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at > NOW()",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    PendingRiskyWorkloadSummary {
        unresolved_incidents,
        high_priority_tickets,
        pending_approvals: pending_approvals_workflow + pending_approvals_playbook,
    }
}

async fn count_reservation_overlaps(
    db: &sqlx::PgPool,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_change_calendar_reservations
         WHERE status = 'reserved'
           AND start_at < $2
           AND end_at > $1
           AND ($3::text IS NULL OR site = $3)
           AND ($4::text IS NULL OR department = $4)",
    )
    .bind(start_at)
    .bind(end_at)
    .bind(site)
    .bind(department)
    .fetch_one(db)
    .await?;
    Ok(count)
}

fn collect_candidate_slot_starts(
    windows: &[MaintenanceWindow],
    timezone: Tz,
    from_time: DateTime<Utc>,
    duration_minutes: i64,
) -> Vec<DateTime<Utc>> {
    if windows.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let local_from = from_time.with_timezone(&timezone).naive_local();
    for day_offset in 0i64..21i64 {
        let candidate_date = local_from.date() + Duration::days(day_offset);
        let weekday = candidate_date.weekday().number_from_monday() as u8;
        for window in windows.iter().filter(|item| item.day_of_week == weekday) {
            let Ok(window_start) = NaiveTime::parse_from_str(window.start.as_str(), "%H:%M") else {
                continue;
            };
            let Ok(window_end) = NaiveTime::parse_from_str(window.end.as_str(), "%H:%M") else {
                continue;
            };
            if window_end <= window_start {
                continue;
            }

            let mut cursor = candidate_date.and_time(window_start);
            let end_bound = candidate_date.and_time(window_end);
            while cursor + Duration::minutes(duration_minutes) <= end_bound {
                if cursor > local_from {
                    let local_dt = match timezone.from_local_datetime(&cursor) {
                        LocalResult::Single(value) => value,
                        LocalResult::Ambiguous(earliest, _) => earliest,
                        LocalResult::None => {
                            cursor += Duration::minutes(60);
                            continue;
                        }
                    };
                    candidates.push(local_dt.with_timezone(&Utc));
                }
                cursor += Duration::minutes(60);
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    candidates
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

fn normalize_risk_level(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => Ok(normalized),
        _ => Err(AppError::Validation(
            "risk_level must be one of: low, medium, high, critical".to_string(),
        )),
    }
}

fn normalize_optional_scope(value: Option<String>) -> Option<String> {
    trim_optional(value, MAX_SCOPE_LEN)
}

fn normalize_optional_status(value: Option<String>) -> AppResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(None);
    }
    match normalized.as_str() {
        "reserved" | "cancelled" => Ok(Some(normalized)),
        _ => Err(AppError::Validation(
            "status must be one of: reserved, cancelled".to_string(),
        )),
    }
}

fn parse_optional_rfc3339(value: Option<String>) -> AppResult<DateTime<Utc>> {
    let Some(raw) = value else {
        return Ok(Utc::now());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Utc::now());
    }
    DateTime::parse_from_rfc3339(trimmed)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| AppError::Validation("from_time must use RFC3339 format".to_string()))
}

fn parse_change_calendar_range(
    start_date_raw: Option<String>,
    end_date_raw: Option<String>,
    days_raw: Option<u32>,
) -> AppResult<(NaiveDate, NaiveDate)> {
    let start_date = start_date_raw
        .map(|value| {
            let trimmed = value.trim().to_string();
            NaiveDate::parse_from_str(trimmed.as_str(), "%Y-%m-%d").map_err(|_| {
                AppError::Validation("start_date must use YYYY-MM-DD format".to_string())
            })
        })
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());

    let end_date = if let Some(raw) = end_date_raw {
        NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
            .map_err(|_| AppError::Validation("end_date must use YYYY-MM-DD format".to_string()))?
    } else {
        let days = days_raw
            .unwrap_or(DEFAULT_RANGE_DAYS)
            .clamp(1, MAX_RANGE_DAYS);
        start_date + Duration::days((days - 1) as i64)
    };

    if end_date < start_date {
        return Err(AppError::Validation(
            "end_date must be equal to or later than start_date".to_string(),
        ));
    }
    Ok((start_date, end_date))
}

fn parse_timezone(value: &str) -> Tz {
    value.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn parse_maintenance_windows(value: Value) -> Vec<MaintenanceWindow> {
    serde_json::from_value::<Vec<MaintenanceWindow>>(value)
        .unwrap_or_default()
        .into_iter()
        .filter(|window| (1..=7).contains(&window.day_of_week))
        .collect()
}

fn build_window_bounds(
    date: NaiveDate,
    window: &MaintenanceWindow,
    timezone: Tz,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let start = NaiveTime::parse_from_str(window.start.as_str(), "%H:%M").ok()?;
    let end = NaiveTime::parse_from_str(window.end.as_str(), "%H:%M").ok()?;
    if end <= start {
        return None;
    }
    let start_local = date.and_time(start);
    let end_local = date.and_time(end);
    let starts_at = match timezone.from_local_datetime(&start_local) {
        LocalResult::Single(value) => value,
        LocalResult::Ambiguous(earliest, _) => earliest,
        LocalResult::None => return None,
    }
    .with_timezone(&Utc);
    let ends_at = match timezone.from_local_datetime(&end_local) {
        LocalResult::Single(value) => value,
        LocalResult::Ambiguous(earliest, _) => earliest,
        LocalResult::None => return None,
    }
    .with_timezone(&Utc);
    Some((starts_at, ends_at))
}

fn slot_is_in_maintenance_window(
    windows: &[MaintenanceWindow],
    timezone: Tz,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
) -> bool {
    let local_start = start_at.with_timezone(&timezone);
    let local_end = end_at.with_timezone(&timezone);
    let weekday = local_start.weekday().number_from_monday() as u8;
    let start_time = local_start.time();
    let end_time = local_end.time();

    windows.iter().any(|window| {
        if window.day_of_week != weekday {
            return false;
        }
        let Ok(window_start) = NaiveTime::parse_from_str(window.start.as_str(), "%H:%M") else {
            return false;
        };
        let Ok(window_end) = NaiveTime::parse_from_str(window.end.as_str(), "%H:%M") else {
            return false;
        };
        window_start <= start_time && end_time <= window_end
    })
}

fn find_next_maintenance_slot(
    windows: &[MaintenanceWindow],
    timezone: Tz,
    from_time: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if windows.is_empty() {
        return None;
    }

    let base_local = from_time.with_timezone(&timezone).naive_local();
    for offset in 0i64..14i64 {
        let candidate_date = base_local.date() + Duration::days(offset);
        let weekday = candidate_date.weekday().number_from_monday() as u8;
        for window in windows.iter().filter(|item| item.day_of_week == weekday) {
            let Ok(start) = NaiveTime::parse_from_str(window.start.as_str(), "%H:%M") else {
                continue;
            };
            let candidate_local = candidate_date.and_time(start);
            if offset == 0 && candidate_local <= base_local {
                continue;
            }
            let candidate = match timezone.from_local_datetime(&candidate_local) {
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(earliest, _) => earliest,
                LocalResult::None => continue,
            };
            return Some(candidate.with_timezone(&Utc));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        ChangeCalendarEvent, MaintenanceWindow, collect_candidate_slot_starts,
        normalize_optional_status, parse_change_calendar_range,
    };
    use chrono::{DateTime, Utc};
    use chrono_tz::UTC as TzUtc;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid RFC3339")
            .with_timezone(&Utc)
    }

    #[test]
    fn parses_change_calendar_range_with_days_fallback() {
        let (start, end) =
            parse_change_calendar_range(Some("2026-03-01".to_string()), None, Some(3))
                .expect("range");
        assert_eq!(start.to_string(), "2026-03-01");
        assert_eq!(end.to_string(), "2026-03-03");
    }

    #[test]
    fn rejects_change_calendar_range_with_reversed_dates() {
        let result = parse_change_calendar_range(
            Some("2026-03-05".to_string()),
            Some("2026-03-03".to_string()),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn change_calendar_sort_order_is_deterministic() {
        let mut items = vec![
            ChangeCalendarEvent {
                event_key: "k2".to_string(),
                event_type: "pending_workflow_approval".to_string(),
                severity: "medium".to_string(),
                title: "workflow".to_string(),
                starts_at: ts("2026-03-01T01:00:00Z"),
                ends_at: ts("2026-03-01T02:00:00Z"),
                source_type: "workflow_request".to_string(),
                source_id: "2".to_string(),
                details: "b".to_string(),
            },
            ChangeCalendarEvent {
                event_key: "k1".to_string(),
                event_type: "maintenance_window".to_string(),
                severity: "medium".to_string(),
                title: "window".to_string(),
                starts_at: ts("2026-03-01T01:00:00Z"),
                ends_at: ts("2026-03-01T02:00:00Z"),
                source_type: "playbook_policy".to_string(),
                source_id: "global".to_string(),
                details: "a".to_string(),
            },
            ChangeCalendarEvent {
                event_key: "k0".to_string(),
                event_type: "scheduled_backup".to_string(),
                severity: "low".to_string(),
                title: "backup".to_string(),
                starts_at: ts("2026-03-01T00:30:00Z"),
                ends_at: ts("2026-03-01T01:00:00Z"),
                source_type: "backup_policy".to_string(),
                source_id: "1".to_string(),
                details: "c".to_string(),
            },
        ];

        items.sort_by(|left, right| {
            left.starts_at
                .cmp(&right.starts_at)
                .then_with(|| left.event_type.cmp(&right.event_type))
                .then_with(|| left.event_key.cmp(&right.event_key))
        });

        let keys = items
            .into_iter()
            .map(|item| item.event_key)
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["k0", "k1", "k2"]);
    }

    #[test]
    fn normalizes_optional_reservation_status_filter() {
        assert_eq!(
            normalize_optional_status(Some(" reserved ".to_string())).expect("status"),
            Some("reserved".to_string())
        );
        assert!(normalize_optional_status(Some("invalid".to_string())).is_err());
    }

    #[test]
    fn slot_recommendation_candidates_are_deterministic() {
        let windows = vec![MaintenanceWindow {
            day_of_week: 1,
            start: "00:00".to_string(),
            end: "03:00".to_string(),
            label: Some("monday-window".to_string()),
        }];
        let from_time = DateTime::parse_from_rfc3339("2026-03-01T23:00:00Z")
            .expect("valid")
            .with_timezone(&Utc);
        let first = collect_candidate_slot_starts(&windows, TzUtc, from_time, 60);
        let second = collect_candidate_slot_starts(&windows, TzUtc, from_time, 60);
        assert_eq!(first, second);
        assert!(!first.is_empty());
    }
}
