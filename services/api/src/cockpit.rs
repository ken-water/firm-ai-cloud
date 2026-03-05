use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{error::AppResult, state::AppState};

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;
const STALE_PENDING_MINUTES: i64 = 15;
const STALE_STREAM_MINUTES: i64 = 20;

pub fn routes() -> Router<AppState> {
    Router::new().route("/cockpit/queue", get(get_daily_cockpit_queue))
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

async fn fetch_alert_queue_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AlertQueueRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, alert_source, alert_key, title, severity, status, site, department, asset_id, last_seen_at
         FROM unified_alerts a
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

            DailyCockpitQueueItem {
                queue_key: format!("alert:{}", row.id),
                item_type: "alert".to_string(),
                priority_score,
                priority_level,
                rationale,
                rationale_details: vec![
                    format!("severity:{}", row.severity),
                    format!("status:{}", row.status),
                    format!("age_minutes:{age_minutes}"),
                ],
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
                        key: "open-playbook-diagnostics".to_string(),
                        label: "Run Diagnostics Playbook".to_string(),
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
    use chrono::{Duration, Utc};

    use super::{
        DailyCockpitAction, DailyCockpitQueueItem, score_alert_item, score_ticket_item,
        sort_daily_queue_items,
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
}
