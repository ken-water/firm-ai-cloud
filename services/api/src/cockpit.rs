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
const CHECKLIST_STATUS_PENDING: &str = "pending";
const CHECKLIST_STATUS_COMPLETED: &str = "completed";
const CHECKLIST_STATUS_SKIPPED: &str = "skipped";
const MAX_CHECKLIST_NOTE_LEN: usize = 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/queue", get(get_daily_cockpit_queue))
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

#[derive(Debug, Serialize)]
struct DailyCockpitQueueResponse {
    generated_at: DateTime<Utc>,
    scope: DailyCockpitScope,
    window: DailyCockpitWindow,
    items: Vec<DailyCockpitQueueItem>,
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

    use super::{
        DailyCockpitAction, DailyCockpitQueueItem, MAX_CHECKLIST_NOTE_LEN, OpsChecklistEntryRow,
        OpsChecklistTemplateRow, build_ops_checklist_response, normalize_optional_note,
        parse_optional_date, score_alert_item, score_ticket_item, sort_daily_queue_items,
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
    fn parses_optional_date_with_expected_format() {
        assert_eq!(
            parse_optional_date(Some("2026-03-06".to_string())).expect("date should parse"),
            NaiveDate::from_ymd_opt(2026, 3, 6).expect("date is valid")
        );
        assert!(parse_optional_date(Some("2026/03/06".to_string())).is_err());
    }
}
