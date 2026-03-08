use std::collections::{BTreeSet, HashSet};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
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
const TICKET_ESCALATION_POLICY_KEY_DEFAULT: &str = "default-ticket-sla";
const MAX_ESCALATION_REASON_LEN: usize = 1_024;
const MAX_ESCALATION_ACTION_LIMIT: u32 = 200;
const DEFAULT_ESCALATION_ACTION_LIMIT: u32 = 60;

const ESCALATION_STATE_NORMAL: &str = "normal";
const ESCALATION_STATE_NEAR_BREACH: &str = "near_breach";
const ESCALATION_STATE_BREACHED: &str = "breached";

const ESCALATION_ACTION_ESCALATED: &str = "escalated";
const ESCALATION_ACTION_SKIPPED: &str = "run_skipped";

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
        .route(
            "/tickets/escalation/policy",
            get(get_ticket_escalation_policy).put(update_ticket_escalation_policy),
        )
        .route(
            "/tickets/escalation/policy/preview",
            post(preview_ticket_escalation_policy),
        )
        .route(
            "/tickets/escalation/queue",
            get(list_ticket_escalation_queue),
        )
        .route(
            "/tickets/escalation/actions",
            get(list_ticket_escalation_actions),
        )
        .route("/tickets/escalation/run", post(run_ticket_escalation))
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
    escalation_state: String,
    escalation_age_minutes: i64,
    escalation_due_at: Option<DateTime<Utc>>,
    escalation_last_action_at: Option<DateTime<Utc>>,
    escalation_last_action_kind: Option<String>,
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
    escalation: TicketEscalationDetail,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
struct TicketEscalationPolicyRecord {
    id: i64,
    policy_key: String,
    name: String,
    is_enabled: bool,
    near_critical_minutes: i32,
    breach_critical_minutes: i32,
    near_high_minutes: i32,
    breach_high_minutes: i32,
    near_medium_minutes: i32,
    breach_medium_minutes: i32,
    near_low_minutes: i32,
    breach_low_minutes: i32,
    escalate_to_assignee: String,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
struct TicketEscalationActionRecord {
    id: i64,
    ticket_id: i64,
    action_kind: String,
    state_before: String,
    state_after: String,
    from_assignee: Option<String>,
    to_assignee: Option<String>,
    actor: String,
    reason: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct TicketEscalationDetail {
    policy_key: String,
    policy_name: String,
    policy_enabled: bool,
    state: String,
    age_minutes: i64,
    near_breach_minutes: i32,
    breach_minutes: i32,
    due_at: Option<DateTime<Utc>>,
    escalate_to_assignee: String,
    latest_action: Option<TicketEscalationActionRecord>,
}

#[derive(Debug, Serialize)]
struct ListTicketEscalationActionsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<TicketEscalationActionRecord>,
}

#[derive(Debug, Serialize)]
struct TicketEscalationPreviewResponse {
    priority: String,
    status: String,
    ticket_age_minutes: i64,
    state: String,
    near_breach_minutes: i32,
    breach_minutes: i32,
    should_escalate: bool,
    escalate_to_assignee: String,
}

#[derive(Debug, Serialize)]
struct TicketEscalationRunResponse {
    generated_at: DateTime<Utc>,
    dry_run: bool,
    policy_key: String,
    processed: usize,
    escalated: usize,
    skipped: usize,
    actions: Vec<TicketEscalationActionRecord>,
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

#[derive(Debug, Deserialize)]
struct UpdateTicketEscalationPolicyRequest {
    name: Option<String>,
    is_enabled: Option<bool>,
    near_critical_minutes: Option<i32>,
    breach_critical_minutes: Option<i32>,
    near_high_minutes: Option<i32>,
    breach_high_minutes: Option<i32>,
    near_medium_minutes: Option<i32>,
    breach_medium_minutes: Option<i32>,
    near_low_minutes: Option<i32>,
    breach_low_minutes: Option<i32>,
    escalate_to_assignee: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TicketEscalationPreviewRequest {
    priority: String,
    status: String,
    ticket_age_minutes: i64,
    current_assignee: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListTicketEscalationQueueQuery {
    state: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ListTicketEscalationActionsQuery {
    ticket_id: Option<i64>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RunTicketEscalationRequest {
    dry_run: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct TicketEscalationEvaluation {
    state: String,
    age_minutes: i64,
    near_breach_minutes: i32,
    breach_minutes: i32,
    due_at: Option<DateTime<Utc>>,
    should_escalate: bool,
}

impl TicketEscalationEvaluation {
    fn with_due_at(mut self, due_at: Option<DateTime<Utc>>) -> Self {
        self.due_at = due_at;
        self
    }
}

async fn list_tickets(
    State(state): State<AppState>,
    Query(query): Query<ListTicketsQuery>,
) -> AppResult<Json<ListTicketsResponse>> {
    let policy = load_ticket_escalation_policy(&state.db).await?;
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
                (SELECT COUNT(*) FROM ticket_alert_links al WHERE al.ticket_id = t.id) AS alert_link_count,
                'normal'::text AS escalation_state,
                0::bigint AS escalation_age_minutes,
                NULL::timestamptz AS escalation_due_at,
                (SELECT a.created_at
                 FROM ticket_escalation_actions a
                 WHERE a.ticket_id = t.id
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT 1) AS escalation_last_action_at,
                (SELECT a.action_kind
                 FROM ticket_escalation_actions a
                 WHERE a.ticket_id = t.id
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT 1) AS escalation_last_action_kind
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
    let now = Utc::now();
    let items = items
        .into_iter()
        .map(|item| apply_ticket_escalation(item, now, &policy))
        .collect();

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

async fn get_ticket_escalation_policy(
    State(state): State<AppState>,
) -> AppResult<Json<TicketEscalationPolicyRecord>> {
    let policy = load_ticket_escalation_policy(&state.db).await?;
    Ok(Json(policy))
}

async fn update_ticket_escalation_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateTicketEscalationPolicyRequest>,
) -> AppResult<Json<TicketEscalationPolicyRecord>> {
    let current = load_ticket_escalation_policy(&state.db).await?;
    let name = trim_optional(payload.name, 128).unwrap_or(current.name.clone());
    let is_enabled = payload.is_enabled.unwrap_or(current.is_enabled);
    let near_critical_minutes = payload
        .near_critical_minutes
        .unwrap_or(current.near_critical_minutes);
    let breach_critical_minutes = payload
        .breach_critical_minutes
        .unwrap_or(current.breach_critical_minutes);
    let near_high_minutes = payload
        .near_high_minutes
        .unwrap_or(current.near_high_minutes);
    let breach_high_minutes = payload
        .breach_high_minutes
        .unwrap_or(current.breach_high_minutes);
    let near_medium_minutes = payload
        .near_medium_minutes
        .unwrap_or(current.near_medium_minutes);
    let breach_medium_minutes = payload
        .breach_medium_minutes
        .unwrap_or(current.breach_medium_minutes);
    let near_low_minutes = payload.near_low_minutes.unwrap_or(current.near_low_minutes);
    let breach_low_minutes = payload
        .breach_low_minutes
        .unwrap_or(current.breach_low_minutes);
    let escalate_to_assignee = trim_optional(payload.escalate_to_assignee, MAX_USER_REF_LEN)
        .unwrap_or(current.escalate_to_assignee.clone());
    let note = trim_optional(payload.note, MAX_ESCALATION_REASON_LEN);

    validate_escalation_pair("critical", near_critical_minutes, breach_critical_minutes)?;
    validate_escalation_pair("high", near_high_minutes, breach_high_minutes)?;
    validate_escalation_pair("medium", near_medium_minutes, breach_medium_minutes)?;
    validate_escalation_pair("low", near_low_minutes, breach_low_minutes)?;
    if escalate_to_assignee.trim().is_empty() {
        return Err(AppError::Validation(
            "escalate_to_assignee is required".to_string(),
        ));
    }

    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());

    let updated: TicketEscalationPolicyRecord = sqlx::query_as(
        "UPDATE ticket_escalation_policies
         SET name = $2,
             is_enabled = $3,
             near_critical_minutes = $4,
             breach_critical_minutes = $5,
             near_high_minutes = $6,
             breach_high_minutes = $7,
             near_medium_minutes = $8,
             breach_medium_minutes = $9,
             near_low_minutes = $10,
             breach_low_minutes = $11,
             escalate_to_assignee = $12,
             updated_by = $13,
             updated_at = NOW()
         WHERE policy_key = $1
         RETURNING id, policy_key, name, is_enabled,
                   near_critical_minutes, breach_critical_minutes,
                   near_high_minutes, breach_high_minutes,
                   near_medium_minutes, breach_medium_minutes,
                   near_low_minutes, breach_low_minutes,
                   escalate_to_assignee, updated_by, created_at, updated_at",
    )
    .bind(TICKET_ESCALATION_POLICY_KEY_DEFAULT)
    .bind(name)
    .bind(is_enabled)
    .bind(near_critical_minutes)
    .bind(breach_critical_minutes)
    .bind(near_high_minutes)
    .bind(breach_high_minutes)
    .bind(near_medium_minutes)
    .bind(breach_medium_minutes)
    .bind(near_low_minutes)
    .bind(breach_low_minutes)
    .bind(escalate_to_assignee)
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "ticket.escalation.policy.update",
        "ticket_escalation_policy",
        Some(updated.id.to_string()),
        "success",
        note,
        json!({
            "policy_key": updated.policy_key,
            "is_enabled": updated.is_enabled,
            "escalate_to_assignee": updated.escalate_to_assignee,
            "updated_by": actor
        }),
    )
    .await;

    Ok(Json(updated))
}

async fn preview_ticket_escalation_policy(
    State(state): State<AppState>,
    Json(payload): Json<TicketEscalationPreviewRequest>,
) -> AppResult<Json<TicketEscalationPreviewResponse>> {
    let policy = load_ticket_escalation_policy(&state.db).await?;
    let priority = normalize_priority(Some(payload.priority))?;
    let status = normalize_required_status(payload.status)?;
    if payload.ticket_age_minutes < 0 {
        return Err(AppError::Validation(
            "ticket_age_minutes must be >= 0".to_string(),
        ));
    }

    let evaluation = evaluate_ticket_escalation_by_age(
        priority.as_str(),
        status.as_str(),
        payload.ticket_age_minutes,
        &policy,
    );

    let should_escalate = evaluation.should_escalate
        && payload
            .current_assignee
            .as_deref()
            .unwrap_or_default()
            .trim()
            != policy.escalate_to_assignee;

    Ok(Json(TicketEscalationPreviewResponse {
        priority,
        status,
        ticket_age_minutes: payload.ticket_age_minutes,
        state: evaluation.state,
        near_breach_minutes: evaluation.near_breach_minutes,
        breach_minutes: evaluation.breach_minutes,
        should_escalate,
        escalate_to_assignee: policy.escalate_to_assignee,
    }))
}

async fn list_ticket_escalation_queue(
    State(state): State<AppState>,
    Query(query): Query<ListTicketEscalationQueueQuery>,
) -> AppResult<Json<ListTicketsResponse>> {
    let policy = load_ticket_escalation_policy(&state.db).await?;
    let limit = query.limit.unwrap_or(60).min(200) as usize;
    let offset = query.offset.unwrap_or(0) as usize;
    let state_filter = query.state.map(normalize_escalation_state).transpose()?;

    let items: Vec<TicketListItem> = sqlx::query_as(
        "SELECT t.id, t.ticket_no, t.title, t.status, t.priority, t.category, t.requester, t.assignee,
                t.workflow_template_id, t.workflow_request_id, t.closed_at, t.created_at, t.updated_at,
                (SELECT COUNT(*) FROM ticket_asset_links ta WHERE ta.ticket_id = t.id) AS asset_link_count,
                (SELECT COUNT(*) FROM ticket_alert_links al WHERE al.ticket_id = t.id) AS alert_link_count,
                'normal'::text AS escalation_state,
                0::bigint AS escalation_age_minutes,
                NULL::timestamptz AS escalation_due_at,
                (SELECT a.created_at
                 FROM ticket_escalation_actions a
                 WHERE a.ticket_id = t.id
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT 1) AS escalation_last_action_at,
                (SELECT a.action_kind
                 FROM ticket_escalation_actions a
                 WHERE a.ticket_id = t.id
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT 1) AS escalation_last_action_kind
         FROM tickets t
         WHERE t.status IN ('open', 'in_progress')
         ORDER BY t.created_at DESC, t.id DESC
         LIMIT 400",
    )
    .fetch_all(&state.db)
    .await?;

    let now = Utc::now();
    let mut filtered: Vec<TicketListItem> = items
        .into_iter()
        .map(|item| apply_ticket_escalation(item, now, &policy))
        .filter(|item| {
            item.escalation_state == ESCALATION_STATE_NEAR_BREACH
                || item.escalation_state == ESCALATION_STATE_BREACHED
        })
        .collect();

    if let Some(state) = state_filter {
        filtered.retain(|item| item.escalation_state == state);
    }

    let total = filtered.len() as i64;
    let page_items = filtered.into_iter().skip(offset).take(limit).collect();

    Ok(Json(ListTicketsResponse {
        items: page_items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn list_ticket_escalation_actions(
    State(state): State<AppState>,
    Query(query): Query<ListTicketEscalationActionsQuery>,
) -> AppResult<Json<ListTicketEscalationActionsResponse>> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_ESCALATION_ACTION_LIMIT)
        .clamp(1, MAX_ESCALATION_ACTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ticket_escalation_actions a WHERE 1=1");
    if let Some(ticket_id) = query.ticket_id {
        count_builder
            .push(" AND a.ticket_id = ")
            .push_bind(ticket_id);
    }
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, ticket_id, action_kind, state_before, state_after, from_assignee, to_assignee,
                actor, reason, created_at
         FROM ticket_escalation_actions a
         WHERE 1=1",
    );
    if let Some(ticket_id) = query.ticket_id {
        list_builder
            .push(" AND a.ticket_id = ")
            .push_bind(ticket_id);
    }
    list_builder
        .push(" ORDER BY a.created_at DESC, a.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<TicketEscalationActionRecord> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(ListTicketEscalationActionsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn run_ticket_escalation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RunTicketEscalationRequest>,
) -> AppResult<Json<TicketEscalationRunResponse>> {
    let policy = load_ticket_escalation_policy(&state.db).await?;
    let dry_run = payload.dry_run.unwrap_or(false);
    let reason = trim_optional(payload.note, MAX_ESCALATION_REASON_LEN);
    let actor = actor_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());

    let mut candidates: Vec<TicketRecord> = sqlx::query_as(
        "SELECT id, ticket_no, title, description, status, priority, category, requester, assignee,
                workflow_template_id, workflow_request_id, metadata, last_status_note, closed_at, created_at, updated_at
         FROM tickets
         WHERE status IN ('open', 'in_progress')
         ORDER BY created_at ASC, id ASC
         LIMIT 500",
    )
    .fetch_all(&state.db)
    .await?;

    let now = Utc::now();
    let mut processed = 0usize;
    let mut escalated = 0usize;
    let mut skipped = 0usize;
    let mut actions: Vec<TicketEscalationActionRecord> = Vec::new();

    let mut tx = state.db.begin().await?;
    for ticket in &mut candidates {
        let evaluation = evaluate_ticket_escalation(
            ticket.priority.as_str(),
            ticket.status.as_str(),
            ticket.created_at,
            now,
            &policy,
        );
        if !evaluation.should_escalate {
            continue;
        }
        processed += 1;

        let current_assignee = ticket.assignee.clone();
        if current_assignee.as_deref() == Some(policy.escalate_to_assignee.as_str()) {
            skipped += 1;
            if !dry_run {
                let action: TicketEscalationActionRecord = sqlx::query_as(
                    "INSERT INTO ticket_escalation_actions (
                        ticket_id, policy_id, action_kind, state_before, state_after,
                        from_assignee, to_assignee, actor, reason, metadata
                     )
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                     RETURNING id, ticket_id, action_kind, state_before, state_after, from_assignee, to_assignee,
                               actor, reason, created_at",
                )
                .bind(ticket.id)
                .bind(policy.id)
                .bind(ESCALATION_ACTION_SKIPPED)
                .bind(evaluation.state.as_str())
                .bind(evaluation.state.as_str())
                .bind(current_assignee.as_deref())
                .bind(current_assignee.as_deref())
                .bind(actor.as_str())
                .bind(reason.as_deref())
                .bind(json!({
                    "dry_run": dry_run,
                    "reason": "already escalated owner"
                }))
                .fetch_one(&mut *tx)
                .await?;
                actions.push(action);
            }
            continue;
        }

        escalated += 1;
        if !dry_run {
            sqlx::query(
                "UPDATE tickets
                 SET assignee = $2,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(ticket.id)
            .bind(policy.escalate_to_assignee.as_str())
            .execute(&mut *tx)
            .await?;

            let action: TicketEscalationActionRecord = sqlx::query_as(
                "INSERT INTO ticket_escalation_actions (
                    ticket_id, policy_id, action_kind, state_before, state_after,
                    from_assignee, to_assignee, actor, reason, metadata
                 )
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 RETURNING id, ticket_id, action_kind, state_before, state_after, from_assignee, to_assignee,
                           actor, reason, created_at",
            )
            .bind(ticket.id)
            .bind(policy.id)
            .bind(ESCALATION_ACTION_ESCALATED)
            .bind(evaluation.state.as_str())
            .bind(ESCALATION_STATE_BREACHED)
            .bind(current_assignee.as_deref())
            .bind(policy.escalate_to_assignee.as_str())
            .bind(actor.as_str())
            .bind(reason.as_deref())
            .bind(json!({
                "dry_run": dry_run,
                "ticket_no": ticket.ticket_no,
                "priority": ticket.priority
            }))
            .fetch_one(&mut *tx)
            .await?;
            actions.push(action);
        }
    }

    if !dry_run {
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "ticket.escalation.run",
        "ticket_escalation_policy",
        Some(policy.id.to_string()),
        "success",
        reason.clone(),
        json!({
            "dry_run": dry_run,
            "processed": processed,
            "escalated": escalated,
            "skipped": skipped
        }),
    )
    .await;

    Ok(Json(TicketEscalationRunResponse {
        generated_at: Utc::now(),
        dry_run,
        policy_key: policy.policy_key,
        processed,
        escalated,
        skipped,
        actions,
    }))
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

    let policy = load_ticket_escalation_policy(db).await?;
    let evaluation = evaluate_ticket_escalation(
        ticket.priority.as_str(),
        ticket.status.as_str(),
        ticket.created_at,
        Utc::now(),
        &policy,
    );
    let latest_action: Option<TicketEscalationActionRecord> = sqlx::query_as(
        "SELECT id, ticket_id, action_kind, state_before, state_after, from_assignee, to_assignee,
                actor, reason, created_at
         FROM ticket_escalation_actions
         WHERE ticket_id = $1
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    Ok(TicketDetailResponse {
        ticket,
        asset_links,
        alert_links,
        escalation: TicketEscalationDetail {
            policy_key: policy.policy_key,
            policy_name: policy.name,
            policy_enabled: policy.is_enabled,
            state: evaluation.state,
            age_minutes: evaluation.age_minutes,
            near_breach_minutes: evaluation.near_breach_minutes,
            breach_minutes: evaluation.breach_minutes,
            due_at: evaluation.due_at,
            escalate_to_assignee: policy.escalate_to_assignee,
            latest_action,
        },
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

fn apply_ticket_escalation(
    mut item: TicketListItem,
    now: DateTime<Utc>,
    policy: &TicketEscalationPolicyRecord,
) -> TicketListItem {
    let evaluation = evaluate_ticket_escalation(
        item.priority.as_str(),
        item.status.as_str(),
        item.created_at,
        now,
        policy,
    );
    item.escalation_state = evaluation.state;
    item.escalation_age_minutes = evaluation.age_minutes;
    item.escalation_due_at = evaluation.due_at;
    item
}

fn evaluate_ticket_escalation(
    priority: &str,
    status: &str,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
    policy: &TicketEscalationPolicyRecord,
) -> TicketEscalationEvaluation {
    let age_minutes = (now - created_at).num_minutes().max(0);
    evaluate_ticket_escalation_by_age(priority, status, age_minutes, policy).with_due_at(
        if matches!(status, STATUS_OPEN | STATUS_IN_PROGRESS) {
            let (_, breach) = escalation_threshold_for_priority(priority, policy);
            Some(created_at + Duration::minutes(breach as i64))
        } else {
            None
        },
    )
}

fn evaluate_ticket_escalation_by_age(
    priority: &str,
    status: &str,
    age_minutes: i64,
    policy: &TicketEscalationPolicyRecord,
) -> TicketEscalationEvaluation {
    let (near_breach_minutes, breach_minutes) = escalation_threshold_for_priority(priority, policy);
    let active_status = matches!(status, STATUS_OPEN | STATUS_IN_PROGRESS);
    if !policy.is_enabled || !active_status {
        return TicketEscalationEvaluation {
            state: ESCALATION_STATE_NORMAL.to_string(),
            age_minutes,
            near_breach_minutes,
            breach_minutes,
            due_at: None,
            should_escalate: false,
        };
    }

    let state = if age_minutes >= breach_minutes as i64 {
        ESCALATION_STATE_BREACHED
    } else if age_minutes >= near_breach_minutes as i64 {
        ESCALATION_STATE_NEAR_BREACH
    } else {
        ESCALATION_STATE_NORMAL
    };
    TicketEscalationEvaluation {
        state: state.to_string(),
        age_minutes,
        near_breach_minutes,
        breach_minutes,
        due_at: None,
        should_escalate: state == ESCALATION_STATE_BREACHED,
    }
}

fn escalation_threshold_for_priority(
    priority: &str,
    policy: &TicketEscalationPolicyRecord,
) -> (i32, i32) {
    match priority {
        PRIORITY_CRITICAL => (policy.near_critical_minutes, policy.breach_critical_minutes),
        PRIORITY_HIGH => (policy.near_high_minutes, policy.breach_high_minutes),
        PRIORITY_LOW => (policy.near_low_minutes, policy.breach_low_minutes),
        _ => (policy.near_medium_minutes, policy.breach_medium_minutes),
    }
}

fn validate_escalation_pair(label: &str, near: i32, breach: i32) -> AppResult<()> {
    if near <= 0 || breach <= 0 {
        return Err(AppError::Validation(format!(
            "{label} escalation windows must be > 0"
        )));
    }
    if near >= breach {
        return Err(AppError::Validation(format!(
            "{label} near window must be less than breach window"
        )));
    }
    Ok(())
}

fn normalize_escalation_state(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        ESCALATION_STATE_NORMAL | ESCALATION_STATE_NEAR_BREACH | ESCALATION_STATE_BREACHED => {
            Ok(normalized)
        }
        _ => Err(AppError::Validation(
            "state must be one of: normal, near_breach, breached".to_string(),
        )),
    }
}

async fn load_ticket_escalation_policy(
    db: &sqlx::PgPool,
) -> AppResult<TicketEscalationPolicyRecord> {
    if let Some(existing) = sqlx::query_as(
        "SELECT id, policy_key, name, is_enabled,
                near_critical_minutes, breach_critical_minutes,
                near_high_minutes, breach_high_minutes,
                near_medium_minutes, breach_medium_minutes,
                near_low_minutes, breach_low_minutes,
                escalate_to_assignee, updated_by, created_at, updated_at
         FROM ticket_escalation_policies
         WHERE policy_key = $1",
    )
    .bind(TICKET_ESCALATION_POLICY_KEY_DEFAULT)
    .fetch_optional(db)
    .await?
    {
        return Ok(existing);
    }

    let inserted: TicketEscalationPolicyRecord = sqlx::query_as(
        "INSERT INTO ticket_escalation_policies (
            policy_key, name, is_enabled,
            near_critical_minutes, breach_critical_minutes,
            near_high_minutes, breach_high_minutes,
            near_medium_minutes, breach_medium_minutes,
            near_low_minutes, breach_low_minutes,
            escalate_to_assignee, updated_by
         )
         VALUES ($1, $2, TRUE, 30, 60, 60, 120, 120, 240, 240, 480, 'ops-escalation', 'system')
         RETURNING id, policy_key, name, is_enabled,
                   near_critical_minutes, breach_critical_minutes,
                   near_high_minutes, breach_high_minutes,
                   near_medium_minutes, breach_medium_minutes,
                   near_low_minutes, breach_low_minutes,
                   escalate_to_assignee, updated_by, created_at, updated_at",
    )
    .bind(TICKET_ESCALATION_POLICY_KEY_DEFAULT)
    .bind("Default Ticket SLA Policy")
    .fetch_one(db)
    .await?;
    Ok(inserted)
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
        ESCALATION_STATE_BREACHED, ESCALATION_STATE_NEAR_BREACH,
        TICKET_ESCALATION_POLICY_KEY_DEFAULT, TicketEscalationPolicyRecord,
        evaluate_ticket_escalation_by_age, normalize_escalation_state, normalize_priority,
        normalize_required_status, supported_priorities, supported_statuses,
        validate_escalation_pair,
    };
    use chrono::Utc;

    fn sample_policy() -> TicketEscalationPolicyRecord {
        TicketEscalationPolicyRecord {
            id: 1,
            policy_key: TICKET_ESCALATION_POLICY_KEY_DEFAULT.to_string(),
            name: "default".to_string(),
            is_enabled: true,
            near_critical_minutes: 30,
            breach_critical_minutes: 60,
            near_high_minutes: 60,
            breach_high_minutes: 120,
            near_medium_minutes: 120,
            breach_medium_minutes: 240,
            near_low_minutes: 240,
            breach_low_minutes: 480,
            escalate_to_assignee: "ops-escalation".to_string(),
            updated_by: "system".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

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

    #[test]
    fn escalation_policy_evaluation_marks_near_breach_and_breach() {
        let policy = sample_policy();
        let near = evaluate_ticket_escalation_by_age("high", "open", 60, &policy);
        assert_eq!(near.state, ESCALATION_STATE_NEAR_BREACH);
        assert!(!near.should_escalate);

        let breached = evaluate_ticket_escalation_by_age("high", "open", 120, &policy);
        assert_eq!(breached.state, ESCALATION_STATE_BREACHED);
        assert!(breached.should_escalate);
    }

    #[test]
    fn escalation_policy_pair_validation_rejects_invalid_window() {
        assert!(validate_escalation_pair("high", 10, 30).is_ok());
        assert!(validate_escalation_pair("high", 10, 10).is_err());
        assert!(validate_escalation_pair("high", 0, 30).is_err());
    }

    #[test]
    fn escalation_state_normalization_supports_expected_values() {
        assert_eq!(
            normalize_escalation_state("near_breach".to_string()).expect("near"),
            "near_breach"
        );
        assert!(normalize_escalation_state("urgent".to_string()).is_err());
    }
}
