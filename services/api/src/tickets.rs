use std::collections::{BTreeSet, HashSet};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};

use crate::{
    audit::{actor_from_headers, write_from_headers_best_effort},
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_TITLE_LEN: usize = 255;
const MAX_DESCRIPTION_LEN: usize = 8_000;
const MAX_USER_REF_LEN: usize = 128;
const MAX_CATEGORY_LEN: usize = 64;
const MAX_STATUS_NOTE_LEN: usize = 1_024;
const MAX_ALERT_SOURCE_LEN: usize = 64;
const MAX_ALERT_KEY_LEN: usize = 255;
const MAX_ALERT_TITLE_LEN: usize = 255;
const MAX_ALERT_SEVERITY_LEN: usize = 32;

const STATUS_OPEN: &str = "open";
const STATUS_IN_PROGRESS: &str = "in_progress";
const STATUS_RESOLVED: &str = "resolved";
const STATUS_CLOSED: &str = "closed";
const STATUS_CANCELLED: &str = "cancelled";

const PRIORITY_LOW: &str = "low";
const PRIORITY_MEDIUM: &str = "medium";
const PRIORITY_HIGH: &str = "high";
const PRIORITY_CRITICAL: &str = "critical";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tickets", get(list_tickets).post(create_ticket))
        .route("/tickets/{id}", get(get_ticket))
        .route(
            "/tickets/{id}/status",
            axum::routing::patch(update_ticket_status),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TicketRecord {
    id: i64,
    ticket_no: String,
    title: String,
    description: Option<String>,
    status: String,
    priority: String,
    category: String,
    requester: String,
    assignee: Option<String>,
    workflow_template_id: Option<i64>,
    workflow_request_id: Option<i64>,
    metadata: Value,
    last_status_note: Option<String>,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TicketListItem {
    id: i64,
    ticket_no: String,
    title: String,
    status: String,
    priority: String,
    category: String,
    requester: String,
    assignee: Option<String>,
    workflow_template_id: Option<i64>,
    workflow_request_id: Option<i64>,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    asset_link_count: i64,
    alert_link_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TicketAssetLink {
    asset_id: i64,
    asset_name: Option<String>,
    asset_class: Option<String>,
    asset_status: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TicketAlertLink {
    alert_source: String,
    alert_key: String,
    alert_title: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Serialize)]
struct TicketDetailResponse {
    ticket: TicketRecord,
    asset_links: Vec<TicketAssetLink>,
    alert_links: Vec<TicketAlertLink>,
}

#[derive(Debug, Serialize)]
struct ListTicketsResponse {
    items: Vec<TicketListItem>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Deserialize)]
struct CreateTicketRequest {
    title: String,
    description: Option<String>,
    requester: Option<String>,
    assignee: Option<String>,
    priority: Option<String>,
    category: Option<String>,
    metadata: Option<Value>,
    asset_ids: Option<Vec<i64>>,
    alert_refs: Option<Vec<CreateTicketAlertRef>>,
    workflow_template_id: Option<i64>,
    trigger_workflow: Option<bool>,
    workflow_payload: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CreateTicketAlertRef {
    source: String,
    alert_key: String,
    alert_title: Option<String>,
    severity: Option<String>,
}

#[derive(Debug)]
struct NormalizedTicketAlertRef {
    source: String,
    alert_key: String,
    alert_title: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTicketStatusRequest {
    status: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListTicketsQuery {
    status: Option<String>,
    priority: Option<String>,
    requester: Option<String>,
    assignee: Option<String>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn list_tickets(
    State(state): State<AppState>,
    Query(query): Query<ListTicketsQuery>,
) -> AppResult<Json<ListTicketsResponse>> {
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let status = normalize_optional_status_filter(query.status)?;
    let priority = normalize_optional_priority_filter(query.priority)?;
    let requester = trim_optional(query.requester, MAX_USER_REF_LEN);
    let assignee = trim_optional(query.assignee, MAX_USER_REF_LEN);
    let query_text = trim_optional(query.query, 128);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM tickets t WHERE 1=1");
    append_ticket_filters(
        &mut count_builder,
        status.clone(),
        priority.clone(),
        requester.clone(),
        assignee.clone(),
        query_text.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT t.id, t.ticket_no, t.title, t.status, t.priority, t.category, t.requester, t.assignee,
                t.workflow_template_id, t.workflow_request_id, t.closed_at, t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM ticket_asset_links ta WHERE ta.ticket_id = t.id) AS asset_link_count,
                (SELECT COUNT(*) FROM ticket_alert_links al WHERE al.ticket_id = t.id) AS alert_link_count
         FROM tickets t
         WHERE 1=1",
    );
    append_ticket_filters(
        &mut list_builder,
        status,
        priority,
        requester,
        assignee,
        query_text,
    );
    list_builder
        .push(" ORDER BY t.created_at DESC, t.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<TicketListItem> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListTicketsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_ticket(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<TicketDetailResponse>> {
    let detail = load_ticket_detail(&state.db, id).await?;
    Ok(Json(detail))
}

async fn create_ticket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTicketRequest>,
) -> AppResult<Json<TicketDetailResponse>> {
    let title = required_trimmed("title", payload.title, MAX_TITLE_LEN)?;
    let description = trim_optional(payload.description, MAX_DESCRIPTION_LEN);
    let priority = normalize_priority(payload.priority)?;
    let category =
        trim_optional(payload.category, MAX_CATEGORY_LEN).unwrap_or_else(|| "general".to_string());
    let requester = trim_optional(payload.requester, MAX_USER_REF_LEN)
        .or_else(|| actor_from_headers(&headers))
        .unwrap_or_else(|| "unknown".to_string());
    let assignee = trim_optional(payload.assignee, MAX_USER_REF_LEN);
    let metadata = normalize_metadata(payload.metadata)?;
    let asset_ids = normalize_asset_ids(payload.asset_ids)?;
    let alert_refs = normalize_alert_refs(payload.alert_refs)?;
    let workflow_template_id =
        normalize_optional_positive_id(payload.workflow_template_id, "workflow_template_id")?;
    let trigger_workflow = payload.trigger_workflow.unwrap_or(false);
    let workflow_payload = normalize_workflow_payload(payload.workflow_payload)?;

    if trigger_workflow && workflow_template_id.is_none() {
        return Err(AppError::Validation(
            "workflow_template_id is required when trigger_workflow=true".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;
    let ticket_id: i64 = sqlx::query_scalar("SELECT nextval('tickets_id_seq')")
        .fetch_one(&mut *tx)
        .await?;
    let ticket_no = format!("TKT-{}-{ticket_id:06}", Utc::now().format("%Y%m%d"));

    if !asset_ids.is_empty() {
        validate_asset_links_exist(&mut tx, &asset_ids).await?;
    }

    if let Some(template_id) = workflow_template_id {
        validate_workflow_template_exists(&mut tx, template_id).await?;
    }

    let ticket: TicketRecord = sqlx::query_as(
        "INSERT INTO tickets (
             id, ticket_no, title, description, status, priority, category, requester, assignee,
             workflow_template_id, metadata
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, ticket_no, title, description, status, priority, category, requester, assignee,
                   workflow_template_id, workflow_request_id, metadata, last_status_note, closed_at, created_at, updated_at",
    )
    .bind(ticket_id)
    .bind(&ticket_no)
    .bind(&title)
    .bind(description)
    .bind(STATUS_OPEN)
    .bind(&priority)
    .bind(&category)
    .bind(&requester)
    .bind(assignee)
    .bind(workflow_template_id)
    .bind(metadata)
    .fetch_one(&mut *tx)
    .await?;

    for asset_id in &asset_ids {
        sqlx::query(
            "INSERT INTO ticket_asset_links (ticket_id, asset_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(ticket.id)
        .bind(*asset_id)
        .execute(&mut *tx)
        .await?;
    }

    for alert in &alert_refs {
        sqlx::query(
            "INSERT INTO ticket_alert_links (ticket_id, alert_source, alert_key, alert_title, severity)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT DO NOTHING",
        )
        .bind(ticket.id)
        .bind(&alert.source)
        .bind(&alert.alert_key)
        .bind(&alert.alert_title)
        .bind(&alert.severity)
        .execute(&mut *tx)
        .await?;
    }

    let mut workflow_request_id: Option<i64> = None;
    if trigger_workflow {
        let template_id = workflow_template_id.expect("validated above");
        let workflow_title = format!("[{}] {}", ticket.ticket_no, ticket.title);
        let request_payload = json!({
            "ticket_id": ticket.id,
            "ticket_no": ticket.ticket_no,
            "category": ticket.category,
            "priority": ticket.priority,
            "metadata": ticket.metadata,
            "payload": workflow_payload
        });

        let created_request_id: i64 = sqlx::query_scalar(
            "INSERT INTO workflow_requests (template_id, title, requester, status, payload)
             VALUES ($1, $2, $3, 'pending_approval', $4)
             RETURNING id",
        )
        .bind(template_id)
        .bind(workflow_title)
        .bind(&requester)
        .bind(request_payload)
        .fetch_one(&mut *tx)
        .await?;
        workflow_request_id = Some(created_request_id);

        sqlx::query(
            "UPDATE tickets
             SET workflow_request_id = $2,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(ticket.id)
        .bind(created_request_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let detail = load_ticket_detail(&state.db, ticket.id).await?;
    write_from_headers_best_effort(
        &state.db,
        &headers,
        "ticket.create",
        "ticket",
        Some(ticket.id.to_string()),
        "success",
        None,
        json!({
            "ticket_no": ticket.ticket_no,
            "priority": ticket.priority,
            "asset_links": asset_ids.len(),
            "alert_links": alert_refs.len(),
            "workflow_template_id": workflow_template_id,
            "workflow_request_id": workflow_request_id
        }),
    )
    .await;

    Ok(Json(detail))
}

async fn update_ticket_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTicketStatusRequest>,
) -> AppResult<Json<TicketDetailResponse>> {
    let status = normalize_required_status(payload.status)?;
    let note = trim_optional(payload.note, MAX_STATUS_NOTE_LEN);

    let updated: Option<TicketRecord> = sqlx::query_as(
        "UPDATE tickets
         SET status = $2,
             last_status_note = $3,
             closed_at = CASE
                 WHEN $2 IN ('resolved', 'closed', 'cancelled') THEN NOW()
                 ELSE NULL
             END,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, ticket_no, title, description, status, priority, category, requester, assignee,
                   workflow_template_id, workflow_request_id, metadata, last_status_note, closed_at, created_at, updated_at",
    )
    .bind(id)
    .bind(&status)
    .bind(note.clone())
    .fetch_optional(&state.db)
    .await?;

    let updated = updated.ok_or_else(|| AppError::NotFound(format!("ticket {id} not found")))?;

    let detail = load_ticket_detail(&state.db, id).await?;
    write_from_headers_best_effort(
        &state.db,
        &headers,
        "ticket.status.update",
        "ticket",
        Some(id.to_string()),
        "success",
        None,
        json!({
            "ticket_no": updated.ticket_no,
            "status": updated.status,
            "note": note
        }),
    )
    .await;

    Ok(Json(detail))
}

async fn load_ticket_detail(db: &sqlx::PgPool, id: i64) -> AppResult<TicketDetailResponse> {
    let ticket: Option<TicketRecord> = sqlx::query_as(
        "SELECT id, ticket_no, title, description, status, priority, category, requester, assignee,
                workflow_template_id, workflow_request_id, metadata, last_status_note, closed_at, created_at, updated_at
         FROM tickets
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    let ticket = ticket.ok_or_else(|| AppError::NotFound(format!("ticket {id} not found")))?;

    let asset_links: Vec<TicketAssetLink> = sqlx::query_as(
        "SELECT l.asset_id, a.name AS asset_name, a.asset_class, a.status AS asset_status
         FROM ticket_asset_links l
         LEFT JOIN assets a ON a.id = l.asset_id
         WHERE l.ticket_id = $1
         ORDER BY l.asset_id ASC",
    )
    .bind(id)
    .fetch_all(db)
    .await?;

    let alert_links: Vec<TicketAlertLink> = sqlx::query_as(
        "SELECT alert_source, alert_key, alert_title, severity
         FROM ticket_alert_links
         WHERE ticket_id = $1
         ORDER BY alert_source ASC, alert_key ASC",
    )
    .bind(id)
    .fetch_all(db)
    .await?;

    Ok(TicketDetailResponse {
        ticket,
        asset_links,
        alert_links,
    })
}

async fn validate_asset_links_exist(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset_ids: &[i64],
) -> AppResult<()> {
    let existing: Vec<i64> = sqlx::query_scalar("SELECT id FROM assets WHERE id = ANY($1)")
        .bind(asset_ids)
        .fetch_all(&mut **tx)
        .await?;
    let found: HashSet<i64> = existing.into_iter().collect();

    let missing: Vec<String> = asset_ids
        .iter()
        .filter(|id| !found.contains(id))
        .map(|id| id.to_string())
        .collect();
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "unknown asset ids: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

async fn validate_workflow_template_exists(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    template_id: i64,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM workflow_templates
            WHERE id = $1
              AND is_enabled = TRUE
        )",
    )
    .bind(template_id)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(AppError::Validation(format!(
            "workflow template {template_id} not found or disabled"
        )));
    }
    Ok(())
}

fn append_ticket_filters(
    builder: &mut QueryBuilder<Postgres>,
    status: Option<String>,
    priority: Option<String>,
    requester: Option<String>,
    assignee: Option<String>,
    query_text: Option<String>,
) {
    if let Some(status) = status {
        builder.push(" AND t.status = ").push_bind(status);
    }
    if let Some(priority) = priority {
        builder.push(" AND t.priority = ").push_bind(priority);
    }
    if let Some(requester) = requester {
        builder
            .push(" AND t.requester ILIKE ")
            .push_bind(format!("%{requester}%"));
    }
    if let Some(assignee) = assignee {
        builder
            .push(" AND t.assignee ILIKE ")
            .push_bind(format!("%{assignee}%"));
    }
    if let Some(query_text) = query_text {
        builder.push(" AND (");
        builder
            .push("t.ticket_no ILIKE ")
            .push_bind(format!("%{query_text}%"));
        builder
            .push(" OR t.title ILIKE ")
            .push_bind(format!("%{query_text}%"));
        builder.push(" OR COALESCE(t.description, '') ILIKE ");
        builder.push_bind(format!("%{query_text}%"));
        builder.push(")");
    }
}

fn normalize_optional_status_filter(value: Option<String>) -> AppResult<Option<String>> {
    value.map(normalize_required_status).transpose()
}

fn normalize_required_status(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("status is required".to_string()));
    }

    if !supported_statuses().contains(&normalized.as_str()) {
        return Err(AppError::Validation(format!(
            "unsupported status '{normalized}', supported: {}",
            supported_statuses().join(", ")
        )));
    }

    Ok(normalized)
}

fn supported_statuses() -> Vec<&'static str> {
    vec![
        STATUS_OPEN,
        STATUS_IN_PROGRESS,
        STATUS_RESOLVED,
        STATUS_CLOSED,
        STATUS_CANCELLED,
    ]
}

fn normalize_optional_priority_filter(value: Option<String>) -> AppResult<Option<String>> {
    value.map(|raw| normalize_priority(Some(raw))).transpose()
}

fn normalize_priority(value: Option<String>) -> AppResult<String> {
    let normalized = value
        .map(|raw| raw.trim().to_ascii_lowercase())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| PRIORITY_MEDIUM.to_string());

    if !supported_priorities().contains(&normalized.as_str()) {
        return Err(AppError::Validation(format!(
            "unsupported priority '{normalized}', supported: {}",
            supported_priorities().join(", ")
        )));
    }

    Ok(normalized)
}

fn supported_priorities() -> Vec<&'static str> {
    vec![
        PRIORITY_LOW,
        PRIORITY_MEDIUM,
        PRIORITY_HIGH,
        PRIORITY_CRITICAL,
    ]
}

fn normalize_metadata(metadata: Option<Value>) -> AppResult<Value> {
    let metadata = metadata.unwrap_or_else(|| Value::Object(Default::default()));
    if !metadata.is_object() {
        return Err(AppError::Validation(
            "metadata must be a JSON object".to_string(),
        ));
    }
    Ok(metadata)
}

fn normalize_workflow_payload(payload: Option<Value>) -> AppResult<Value> {
    let payload = payload.unwrap_or_else(|| Value::Object(Default::default()));
    if !payload.is_object() {
        return Err(AppError::Validation(
            "workflow_payload must be a JSON object".to_string(),
        ));
    }
    Ok(payload)
}

fn normalize_optional_positive_id(value: Option<i64>, field: &str) -> AppResult<Option<i64>> {
    match value {
        None => Ok(None),
        Some(id) if id > 0 => Ok(Some(id)),
        Some(_) => Err(AppError::Validation(format!("{field} must be positive"))),
    }
}

fn normalize_asset_ids(asset_ids: Option<Vec<i64>>) -> AppResult<Vec<i64>> {
    let Some(asset_ids) = asset_ids else {
        return Ok(Vec::new());
    };
    let mut deduplicated = BTreeSet::new();
    for asset_id in asset_ids {
        if asset_id <= 0 {
            return Err(AppError::Validation(
                "asset_ids must contain positive integers".to_string(),
            ));
        }
        deduplicated.insert(asset_id);
    }
    Ok(deduplicated.into_iter().collect())
}

fn normalize_alert_refs(
    refs: Option<Vec<CreateTicketAlertRef>>,
) -> AppResult<Vec<NormalizedTicketAlertRef>> {
    let Some(refs) = refs else {
        return Ok(Vec::new());
    };

    let mut unique = HashSet::new();
    let mut normalized = Vec::with_capacity(refs.len());
    for item in refs {
        let source = required_trimmed("alert_refs[].source", item.source, MAX_ALERT_SOURCE_LEN)?
            .to_ascii_lowercase();
        let alert_key =
            required_trimmed("alert_refs[].alert_key", item.alert_key, MAX_ALERT_KEY_LEN)?;
        let dedup_key = format!("{source}:{alert_key}");
        if !unique.insert(dedup_key) {
            continue;
        }

        normalized.push(NormalizedTicketAlertRef {
            source,
            alert_key,
            alert_title: trim_optional(item.alert_title, MAX_ALERT_TITLE_LEN),
            severity: trim_optional(item.severity, MAX_ALERT_SEVERITY_LEN)
                .map(|value| value.to_ascii_lowercase()),
        });
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
            "{field} exceeds max length {max_len}"
        )));
    }
    Ok(trimmed.to_string())
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
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
    use super::{
        normalize_priority, normalize_required_status, supported_priorities, supported_statuses,
    };

    #[test]
    fn normalizes_ticket_status() {
        assert_eq!(
            normalize_required_status("OPEN".to_string()).expect("status"),
            "open"
        );
        assert!(normalize_required_status("".to_string()).is_err());
        assert!(normalize_required_status("unknown".to_string()).is_err());
    }

    #[test]
    fn normalizes_ticket_priority() {
        assert_eq!(
            normalize_priority(Some("HIGH".to_string())).expect("priority"),
            "high"
        );
        assert_eq!(normalize_priority(None).expect("default"), "medium");
        assert!(normalize_priority(Some("urgent".to_string())).is_err());
    }

    #[test]
    fn status_and_priority_sets_are_non_empty() {
        assert!(!supported_statuses().is_empty());
        assert!(!supported_priorities().is_empty());
    }
}
