use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use tracing::warn;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const ALERT_SOURCE_MONITORING_SYNC: &str = "monitoring_sync";
const ALERT_STATUS_OPEN: &str = "open";
const ALERT_STATUS_ACK: &str = "acknowledged";
const ALERT_STATUS_CLOSED: &str = "closed";
const ALERT_SEVERITY_CRITICAL: &str = "critical";
const ALERT_SEVERITY_WARNING: &str = "warning";
const ALERT_SEVERITY_INFO: &str = "info";
const SYSTEM_ACTOR: &str = "system:monitoring-sync";

const POLICY_ACTION_TICKET_CREATED: &str = "ticket_created";
const POLICY_ACTION_SUPPRESSED: &str = "suppressed";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_alerts))
        .route("/{id}", get(get_alert_detail))
        .route("/{id}/ack", axum::routing::post(acknowledge_alert))
        .route("/{id}/close", axum::routing::post(close_alert))
        .route("/bulk/ack", axum::routing::post(bulk_acknowledge_alerts))
        .route("/bulk/close", axum::routing::post(bulk_close_alerts))
        .route(
            "/policies",
            get(list_alert_ticket_policies).post(create_alert_ticket_policy),
        )
        .route(
            "/policies/{id}",
            axum::routing::patch(update_alert_ticket_policy),
        )
}

#[derive(Debug, Serialize, FromRow)]
struct AlertRecord {
    id: i64,
    alert_source: String,
    alert_key: String,
    dedup_key: String,
    title: String,
    severity: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    asset_id: Option<i64>,
    payload: Value,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    acknowledged_by: Option<String>,
    acknowledged_at: Option<DateTime<Utc>>,
    closed_by: Option<String>,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct AlertTimelineRecord {
    id: i64,
    alert_id: i64,
    event_type: String,
    actor: String,
    message: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct AlertLinkedTicketRecord {
    id: i64,
    ticket_no: String,
    title: String,
    status: String,
    priority: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListAlertsResponse {
    items: Vec<AlertRecord>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct AlertDetailResponse {
    alert: AlertRecord,
    timeline: Vec<AlertTimelineRecord>,
    linked_tickets: Vec<AlertLinkedTicketRecord>,
}

#[derive(Debug, Deserialize, Default)]
struct ListAlertsQuery {
    status: Option<String>,
    severity: Option<String>,
    source: Option<String>,
    site: Option<String>,
    department: Option<String>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AlertActionRequest {
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BulkAlertActionRequest {
    ids: Vec<i64>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct BulkAlertActionResponse {
    action: String,
    requested: usize,
    updated: usize,
    skipped: usize,
    updated_ids: Vec<i64>,
    skipped_ids: Vec<i64>,
}

#[derive(Debug, Serialize, FromRow)]
struct AlertTicketPolicyRecord {
    id: i64,
    policy_key: String,
    name: String,
    description: Option<String>,
    is_system: bool,
    is_enabled: bool,
    match_source: Option<String>,
    match_severity: Option<String>,
    match_site: Option<String>,
    match_department: Option<String>,
    match_status: Option<String>,
    dedup_window_seconds: i32,
    ticket_priority: String,
    ticket_category: String,
    workflow_template_id: Option<i64>,
    created_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListAlertTicketPoliciesResponse {
    items: Vec<AlertTicketPolicyRecord>,
    total: usize,
}

#[derive(Debug, Deserialize, Default)]
struct ListAlertTicketPoliciesQuery {
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateAlertTicketPolicyRequest {
    policy_key: String,
    name: String,
    description: Option<String>,
    is_enabled: Option<bool>,
    match_source: Option<String>,
    match_severity: Option<String>,
    match_site: Option<String>,
    match_department: Option<String>,
    match_status: Option<String>,
    dedup_window_seconds: Option<i32>,
    ticket_priority: Option<String>,
    ticket_category: Option<String>,
    workflow_template_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateAlertTicketPolicyRequest {
    name: Option<String>,
    description: Option<String>,
    is_enabled: Option<bool>,
    match_source: Option<String>,
    match_severity: Option<String>,
    match_site: Option<String>,
    match_department: Option<String>,
    match_status: Option<String>,
    dedup_window_seconds: Option<i32>,
    ticket_priority: Option<String>,
    ticket_category: Option<String>,
    workflow_template_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct AlertPolicyMatchContext {
    alert_id: i64,
    alert_source: String,
    alert_key: String,
    title: String,
    severity: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, FromRow)]
struct PolicyActionTsRow {
    created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertStatus {
    Acknowledged,
    Closed,
}

impl AlertStatus {
    fn as_str(self) -> &'static str {
        match self {
            AlertStatus::Acknowledged => ALERT_STATUS_ACK,
            AlertStatus::Closed => ALERT_STATUS_CLOSED,
        }
    }
}

#[derive(Debug)]
struct MonitoringAlertAssetContext {
    name: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug)]
struct AlertUpsertResult {
    id: i64,
    previous_status: Option<String>,
}

#[derive(Debug, FromRow)]
struct TicketSeedRow {
    id: i64,
}

pub(crate) async fn upsert_monitoring_sync_alert(
    state: &AppState,
    job_id: i64,
    asset_id: i64,
    trigger_source: &str,
    severity: &str,
    message: &str,
) -> AppResult<Option<i64>> {
    let severity = normalize_required_severity(Some(severity.to_string()))?;
    let alert_source = ALERT_SOURCE_MONITORING_SYNC.to_string();
    let alert_key = format!("asset:{asset_id}:monitoring-sync");
    let dedup_key = alert_key.clone();

    let asset_ctx = load_monitoring_asset_context(&state.db, asset_id).await?;
    let title = format!("Monitoring sync issue: {}", asset_ctx.name);
    let payload = json!({
        "job_id": job_id,
        "asset_id": asset_id,
        "trigger_source": trigger_source,
        "message": truncate_message(message, 1024),
    });

    let upserted = upsert_alert_record(
        state,
        &alert_source,
        &alert_key,
        &dedup_key,
        &title,
        severity.as_str(),
        asset_ctx.site.as_deref(),
        asset_ctx.department.as_deref(),
        Some(asset_id),
        payload,
    )
    .await?;

    let event_type = if matches!(
        upserted.previous_status.as_deref(),
        Some(ALERT_STATUS_CLOSED) | Some(ALERT_STATUS_ACK)
    ) {
        "reopened"
    } else {
        "observed"
    };

    insert_alert_timeline(
        &state.db,
        upserted.id,
        event_type,
        SYSTEM_ACTOR,
        Some(truncate_message(message, 1024).to_string()),
        json!({
            "job_id": job_id,
            "asset_id": asset_id,
            "trigger_source": trigger_source,
            "severity": severity,
        }),
    )
    .await?;

    evaluate_alert_ticket_policies(state, upserted.id, SYSTEM_ACTOR).await;

    Ok(Some(upserted.id))
}

pub(crate) async fn close_monitoring_sync_alert(
    state: &AppState,
    job_id: i64,
    asset_id: i64,
    message: &str,
) -> AppResult<()> {
    let source = ALERT_SOURCE_MONITORING_SYNC;
    let key = format!("asset:{asset_id}:monitoring-sync");

    let updated: Option<AlertRecord> = sqlx::query_as(
        "UPDATE unified_alerts
         SET status = $3,
             closed_by = $4,
             closed_at = NOW(),
             updated_at = NOW(),
             payload = payload || $5::jsonb,
             last_seen_at = NOW()
         WHERE alert_source = $1
           AND alert_key = $2
           AND status <> 'closed'
         RETURNING id, alert_source, alert_key, dedup_key, title, severity, status, site, department, asset_id,
                   payload, first_seen_at, last_seen_at, acknowledged_by, acknowledged_at, closed_by, closed_at,
                   created_at, updated_at",
    )
    .bind(source)
    .bind(&key)
    .bind(ALERT_STATUS_CLOSED)
    .bind(SYSTEM_ACTOR)
    .bind(json!({
        "resolved_job_id": job_id,
        "resolution_message": truncate_message(message, 1024)
    }))
    .fetch_optional(&state.db)
    .await?;

    if let Some(alert) = updated {
        insert_alert_timeline(
            &state.db,
            alert.id,
            "closed",
            SYSTEM_ACTOR,
            Some("Monitoring sync recovered; alert auto-closed.".to_string()),
            json!({
                "job_id": job_id,
                "asset_id": asset_id,
                "reason": truncate_message(message, 1024)
            }),
        )
        .await?;
    }

    Ok(())
}

async fn list_alerts(
    State(state): State<AppState>,
    Query(query): Query<ListAlertsQuery>,
) -> AppResult<Json<ListAlertsResponse>> {
    let limit = query.limit.unwrap_or(50).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let status = normalize_optional_status_filter(query.status)?;
    let severity = normalize_optional_severity_filter(query.severity)?;
    let source = trim_optional(query.source, 64);
    let site = trim_optional(query.site, 128);
    let department = trim_optional(query.department, 128);
    let query_text = trim_optional(query.query, 128);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM unified_alerts a WHERE 1=1");
    append_alert_filters(
        &mut count_builder,
        status.clone(),
        severity.clone(),
        source.clone(),
        site.clone(),
        department.clone(),
        query_text.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, alert_source, alert_key, dedup_key, title, severity, status, site, department, asset_id,
                payload, first_seen_at, last_seen_at, acknowledged_by, acknowledged_at, closed_by, closed_at,
                created_at, updated_at
         FROM unified_alerts a
         WHERE 1=1",
    );
    append_alert_filters(
        &mut list_builder,
        status,
        severity,
        source,
        site,
        department,
        query_text,
    );
    list_builder
        .push(" ORDER BY a.last_seen_at DESC, a.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<AlertRecord> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListAlertsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_alert_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<AlertDetailResponse>> {
    let alert: Option<AlertRecord> = sqlx::query_as(
        "SELECT id, alert_source, alert_key, dedup_key, title, severity, status, site, department, asset_id,
                payload, first_seen_at, last_seen_at, acknowledged_by, acknowledged_at, closed_by, closed_at,
                created_at, updated_at
         FROM unified_alerts
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let alert = alert.ok_or_else(|| AppError::NotFound(format!("alert {id} not found")))?;

    let timeline: Vec<AlertTimelineRecord> = sqlx::query_as(
        "SELECT id, alert_id, event_type, actor, message, metadata, created_at
         FROM unified_alert_timeline
         WHERE alert_id = $1
         ORDER BY created_at DESC, id DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let linked_tickets: Vec<AlertLinkedTicketRecord> = sqlx::query_as(
        "SELECT t.id, t.ticket_no, t.title, t.status, t.priority, t.created_at
         FROM ticket_alert_links l
         INNER JOIN tickets t ON t.id = l.ticket_id
         WHERE l.alert_source = $1
           AND l.alert_key = $2
         ORDER BY t.created_at DESC, t.id DESC",
    )
    .bind(&alert.alert_source)
    .bind(&alert.alert_key)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(AlertDetailResponse {
        alert,
        timeline,
        linked_tickets,
    }))
}

async fn acknowledge_alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<AlertActionRequest>,
) -> AppResult<Json<AlertRecord>> {
    let actor = resolve_actor(&state, &headers).await;
    let note = trim_optional(payload.note, 1024);

    let updated = update_alert_status(
        &state,
        id,
        AlertStatus::Acknowledged,
        &actor,
        note.as_deref(),
    )
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "alert.acknowledge".to_string(),
            target_type: "alert".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({ "status": updated.status }),
        },
    )
    .await;

    Ok(Json(updated))
}

async fn close_alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<AlertActionRequest>,
) -> AppResult<Json<AlertRecord>> {
    let actor = resolve_actor(&state, &headers).await;
    let note = trim_optional(payload.note, 1024);

    let updated =
        update_alert_status(&state, id, AlertStatus::Closed, &actor, note.as_deref()).await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "alert.close".to_string(),
            target_type: "alert".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({ "status": updated.status }),
        },
    )
    .await;

    Ok(Json(updated))
}

async fn bulk_acknowledge_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BulkAlertActionRequest>,
) -> AppResult<Json<BulkAlertActionResponse>> {
    run_bulk_alert_action(&state, &headers, payload, AlertStatus::Acknowledged).await
}

async fn bulk_close_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BulkAlertActionRequest>,
) -> AppResult<Json<BulkAlertActionResponse>> {
    run_bulk_alert_action(&state, &headers, payload, AlertStatus::Closed).await
}

async fn run_bulk_alert_action(
    state: &AppState,
    headers: &HeaderMap,
    payload: BulkAlertActionRequest,
    target_status: AlertStatus,
) -> AppResult<Json<BulkAlertActionResponse>> {
    let ids = normalize_alert_ids(payload.ids)?;
    let note = trim_optional(payload.note, 1024);
    let actor = resolve_actor(state, headers).await;

    let mut updated_ids = Vec::new();
    let mut skipped_ids = Vec::new();

    for id in &ids {
        match update_alert_status(state, *id, target_status, &actor, note.as_deref()).await {
            Ok(_) => updated_ids.push(*id),
            Err(AppError::NotFound(_)) => skipped_ids.push(*id),
            Err(err) => return Err(err),
        }
    }

    let action = if target_status == AlertStatus::Acknowledged {
        "alert.bulk_acknowledge"
    } else {
        "alert.bulk_close"
    };

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: action.to_string(),
            target_type: "alert".to_string(),
            target_id: None,
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "requested": ids.len(),
                "updated_ids": updated_ids,
                "skipped_ids": skipped_ids,
                "target_status": target_status.as_str(),
            }),
        },
    )
    .await;

    Ok(Json(BulkAlertActionResponse {
        action: target_status.as_str().to_string(),
        requested: ids.len(),
        updated: updated_ids.len(),
        skipped: skipped_ids.len(),
        updated_ids,
        skipped_ids,
    }))
}

async fn list_alert_ticket_policies(
    State(state): State<AppState>,
    Query(query): Query<ListAlertTicketPoliciesQuery>,
) -> AppResult<Json<ListAlertTicketPoliciesResponse>> {
    let items: Vec<AlertTicketPolicyRecord> = if let Some(is_enabled) = query.is_enabled {
        sqlx::query_as(
            "SELECT id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                    match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                    ticket_category, workflow_template_id, created_by, created_at, updated_at
             FROM alert_ticket_policies
             WHERE is_enabled = $1
             ORDER BY is_system DESC, policy_key ASC",
        )
        .bind(is_enabled)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                    match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                    ticket_category, workflow_template_id, created_by, created_at, updated_at
             FROM alert_ticket_policies
             ORDER BY is_system DESC, policy_key ASC",
        )
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(ListAlertTicketPoliciesResponse {
        total: items.len(),
        items,
    }))
}

async fn create_alert_ticket_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAlertTicketPolicyRequest>,
) -> AppResult<Json<AlertTicketPolicyRecord>> {
    let actor = resolve_actor(&state, &headers).await;

    let policy_key = required_trimmed("policy_key", payload.policy_key, 64)?;
    let name = required_trimmed("name", payload.name, 128)?;
    let description = trim_optional(payload.description, 512);
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let match_source = trim_optional(payload.match_source, 64);
    let match_severity = normalize_optional_severity_filter(payload.match_severity)?;
    let match_site = trim_optional(payload.match_site, 128);
    let match_department = trim_optional(payload.match_department, 128);
    let match_status = normalize_optional_status_filter(payload.match_status)?;
    let dedup_window_seconds = payload.dedup_window_seconds.unwrap_or(1800);
    validate_dedup_window(dedup_window_seconds)?;

    let ticket_priority = normalize_priority(payload.ticket_priority)?;
    let ticket_category =
        trim_optional(payload.ticket_category, 64).unwrap_or_else(|| "incident".to_string());
    let workflow_template_id =
        normalize_optional_positive_id(payload.workflow_template_id, "workflow_template_id")?;

    let item: AlertTicketPolicyRecord = sqlx::query_as(
        "INSERT INTO alert_ticket_policies (
            policy_key, name, description, is_system, is_enabled,
            match_source, match_severity, match_site, match_department, match_status,
            dedup_window_seconds, ticket_priority, ticket_category, workflow_template_id, created_by
         )
         VALUES ($1, $2, $3, FALSE, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         RETURNING id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                   match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                   ticket_category, workflow_template_id, created_by, created_at, updated_at",
    )
    .bind(policy_key)
    .bind(name)
    .bind(description)
    .bind(is_enabled)
    .bind(match_source)
    .bind(match_severity)
    .bind(match_site)
    .bind(match_department)
    .bind(match_status)
    .bind(dedup_window_seconds)
    .bind(ticket_priority)
    .bind(ticket_category)
    .bind(workflow_template_id)
    .bind(actor.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "alert.policy.create".to_string(),
            target_type: "alert_policy".to_string(),
            target_id: Some(item.id.to_string()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "policy_key": item.policy_key,
                "is_enabled": item.is_enabled,
                "match_source": item.match_source,
                "match_severity": item.match_severity,
            }),
        },
    )
    .await;

    Ok(Json(item))
}

async fn update_alert_ticket_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateAlertTicketPolicyRequest>,
) -> AppResult<Json<AlertTicketPolicyRecord>> {
    let actor = resolve_actor(&state, &headers).await;

    let current: Option<AlertTicketPolicyRecord> = sqlx::query_as(
        "SELECT id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                ticket_category, workflow_template_id, created_by, created_at, updated_at
         FROM alert_ticket_policies
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let current =
        current.ok_or_else(|| AppError::NotFound(format!("alert policy {id} not found")))?;

    let name = payload
        .name
        .map(|value| required_trimmed("name", value, 128))
        .transpose()?
        .unwrap_or(current.name.clone());

    let description = payload
        .description
        .map(|value| trim_optional(Some(value), 512).unwrap_or_default());
    let description = description.or(current.description.clone());

    let is_enabled = payload.is_enabled.unwrap_or(current.is_enabled);
    let match_source = payload
        .match_source
        .map(|value| trim_optional(Some(value), 64))
        .unwrap_or(current.match_source.clone());
    let match_severity = if let Some(value) = payload.match_severity {
        normalize_optional_severity_filter(Some(value))?
    } else {
        current.match_severity.clone()
    };
    let match_site = payload
        .match_site
        .map(|value| trim_optional(Some(value), 128))
        .unwrap_or(current.match_site.clone());
    let match_department = payload
        .match_department
        .map(|value| trim_optional(Some(value), 128))
        .unwrap_or(current.match_department.clone());
    let match_status = if let Some(value) = payload.match_status {
        normalize_optional_status_filter(Some(value))?
    } else {
        current.match_status.clone()
    };

    let dedup_window_seconds = payload
        .dedup_window_seconds
        .unwrap_or(current.dedup_window_seconds);
    validate_dedup_window(dedup_window_seconds)?;

    let ticket_priority = payload
        .ticket_priority
        .map(|value| normalize_priority(Some(value)))
        .transpose()?
        .unwrap_or(current.ticket_priority.clone());

    let ticket_category = payload
        .ticket_category
        .map(|value| trim_optional(Some(value), 64).unwrap_or_else(|| "incident".to_string()))
        .unwrap_or(current.ticket_category.clone());

    let workflow_template_id = if payload.workflow_template_id.is_some() {
        normalize_optional_positive_id(payload.workflow_template_id, "workflow_template_id")?
    } else {
        current.workflow_template_id
    };

    let updated: AlertTicketPolicyRecord = sqlx::query_as(
        "UPDATE alert_ticket_policies
         SET name = $2,
             description = $3,
             is_enabled = $4,
             match_source = $5,
             match_severity = $6,
             match_site = $7,
             match_department = $8,
             match_status = $9,
             dedup_window_seconds = $10,
             ticket_priority = $11,
             ticket_category = $12,
             workflow_template_id = $13,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                   match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                   ticket_category, workflow_template_id, created_by, created_at, updated_at",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(is_enabled)
    .bind(match_source)
    .bind(match_severity)
    .bind(match_site)
    .bind(match_department)
    .bind(match_status)
    .bind(dedup_window_seconds)
    .bind(ticket_priority)
    .bind(ticket_category)
    .bind(workflow_template_id)
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "alert.policy.update".to_string(),
            target_type: "alert_policy".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "policy_key": updated.policy_key,
                "is_enabled": updated.is_enabled,
                "match_source": updated.match_source,
                "match_severity": updated.match_severity,
                "match_status": updated.match_status,
            }),
        },
    )
    .await;

    Ok(Json(updated))
}

async fn update_alert_status(
    state: &AppState,
    id: i64,
    target_status: AlertStatus,
    actor: &str,
    note: Option<&str>,
) -> AppResult<AlertRecord> {
    let updated: Option<AlertRecord> = match target_status {
        AlertStatus::Acknowledged => {
            sqlx::query_as(
                "UPDATE unified_alerts
                 SET status = $2,
                     acknowledged_by = $3,
                     acknowledged_at = NOW(),
                     closed_by = NULL,
                     closed_at = NULL,
                     updated_at = NOW()
                 WHERE id = $1
                   AND status <> 'closed'
                 RETURNING id, alert_source, alert_key, dedup_key, title, severity, status, site, department, asset_id,
                           payload, first_seen_at, last_seen_at, acknowledged_by, acknowledged_at, closed_by, closed_at,
                           created_at, updated_at",
            )
            .bind(id)
            .bind(ALERT_STATUS_ACK)
            .bind(actor)
            .fetch_optional(&state.db)
            .await?
        }
        AlertStatus::Closed => {
            sqlx::query_as(
                "UPDATE unified_alerts
                 SET status = $2,
                     closed_by = $3,
                     closed_at = NOW(),
                     updated_at = NOW()
                 WHERE id = $1
                   AND status <> 'closed'
                 RETURNING id, alert_source, alert_key, dedup_key, title, severity, status, site, department, asset_id,
                           payload, first_seen_at, last_seen_at, acknowledged_by, acknowledged_at, closed_by, closed_at,
                           created_at, updated_at",
            )
            .bind(id)
            .bind(ALERT_STATUS_CLOSED)
            .bind(actor)
            .fetch_optional(&state.db)
            .await?
        }
    };

    let updated = updated.ok_or_else(|| AppError::NotFound(format!("alert {id} not found")))?;

    let event_type = if target_status == AlertStatus::Acknowledged {
        "acknowledged"
    } else {
        "closed"
    };

    insert_alert_timeline(
        &state.db,
        id,
        event_type,
        actor,
        note.map(ToString::to_string),
        json!({
            "status": updated.status,
        }),
    )
    .await?;

    Ok(updated)
}

async fn upsert_alert_record(
    state: &AppState,
    alert_source: &str,
    alert_key: &str,
    dedup_key: &str,
    title: &str,
    severity: &str,
    site: Option<&str>,
    department: Option<&str>,
    asset_id: Option<i64>,
    payload: Value,
) -> AppResult<AlertUpsertResult> {
    let mut tx = state.db.begin().await?;

    let existing: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, status
         FROM unified_alerts
         WHERE alert_source = $1
           AND alert_key = $2
         FOR UPDATE",
    )
    .bind(alert_source)
    .bind(alert_key)
    .fetch_optional(&mut *tx)
    .await?;

    match existing {
        Some((id, previous_status)) => {
            let reopen =
                previous_status == ALERT_STATUS_CLOSED || previous_status == ALERT_STATUS_ACK;
            let next_status = if reopen {
                ALERT_STATUS_OPEN
            } else {
                previous_status.as_str()
            };

            sqlx::query(
                "UPDATE unified_alerts
                 SET dedup_key = $2,
                     title = $3,
                     severity = $4,
                     status = $5,
                     site = COALESCE($6, site),
                     department = COALESCE($7, department),
                     asset_id = COALESCE($8, asset_id),
                     payload = $9,
                     last_seen_at = NOW(),
                     acknowledged_by = CASE WHEN $10 THEN NULL ELSE acknowledged_by END,
                     acknowledged_at = CASE WHEN $10 THEN NULL ELSE acknowledged_at END,
                     closed_by = CASE WHEN $10 THEN NULL ELSE closed_by END,
                     closed_at = CASE WHEN $10 THEN NULL ELSE closed_at END,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(id)
            .bind(dedup_key)
            .bind(title)
            .bind(severity)
            .bind(next_status)
            .bind(site)
            .bind(department)
            .bind(asset_id)
            .bind(payload)
            .bind(reopen)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(AlertUpsertResult {
                id,
                previous_status: Some(previous_status),
            })
        }
        None => {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO unified_alerts (
                     alert_source, alert_key, dedup_key, title, severity, status,
                     site, department, asset_id, payload
                 )
                 VALUES ($1, $2, $3, $4, $5, 'open', $6, $7, $8, $9)
                 RETURNING id",
            )
            .bind(alert_source)
            .bind(alert_key)
            .bind(dedup_key)
            .bind(title)
            .bind(severity)
            .bind(site)
            .bind(department)
            .bind(asset_id)
            .bind(payload)
            .fetch_one(&mut *tx)
            .await?;

            tx.commit().await?;
            Ok(AlertUpsertResult {
                id,
                previous_status: None,
            })
        }
    }
}

async fn insert_alert_timeline(
    db: &sqlx::PgPool,
    alert_id: i64,
    event_type: &str,
    actor: &str,
    message: Option<String>,
    metadata: Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO unified_alert_timeline (alert_id, event_type, actor, message, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(alert_id)
    .bind(event_type)
    .bind(actor)
    .bind(message)
    .bind(metadata)
    .execute(db)
    .await?;
    Ok(())
}

async fn evaluate_alert_ticket_policies(state: &AppState, alert_id: i64, actor: &str) {
    if let Err(err) = evaluate_alert_ticket_policies_inner(state, alert_id, actor).await {
        warn!(error = %err, alert_id, "failed to evaluate alert ticket policies");
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: actor.to_string(),
                action: "alert.policy.evaluate".to_string(),
                target_type: "alert".to_string(),
                target_id: Some(alert_id.to_string()),
                result: "failed".to_string(),
                message: Some(err.to_string()),
                metadata: json!({ "alert_id": alert_id }),
            },
        )
        .await;
    }
}

async fn evaluate_alert_ticket_policies_inner(
    state: &AppState,
    alert_id: i64,
    actor: &str,
) -> AppResult<()> {
    let alert: Option<AlertPolicyMatchContext> = sqlx::query_as(
        "SELECT id AS alert_id, alert_source, alert_key, title, severity, status, site, department
         FROM unified_alerts
         WHERE id = $1",
    )
    .bind(alert_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(alert) = alert else {
        return Ok(());
    };

    if alert.status != ALERT_STATUS_OPEN {
        return Ok(());
    }

    let policies: Vec<AlertTicketPolicyRecord> = sqlx::query_as(
        "SELECT id, policy_key, name, description, is_system, is_enabled, match_source, match_severity,
                match_site, match_department, match_status, dedup_window_seconds, ticket_priority,
                ticket_category, workflow_template_id, created_by, created_at, updated_at
         FROM alert_ticket_policies
         WHERE is_enabled = TRUE
         ORDER BY is_system DESC, policy_key ASC",
    )
    .fetch_all(&state.db)
    .await?;

    for policy in policies {
        if !policy_matches_alert(&policy, &alert) {
            continue;
        }

        let last_action: PolicyActionTsRow = sqlx::query_as(
            "SELECT MAX(created_at) AS created_at
             FROM alert_policy_actions
             WHERE policy_id = $1
               AND alert_id = $2
               AND action = 'ticket_created'",
        )
        .bind(policy.id)
        .bind(alert.alert_id)
        .fetch_one(&state.db)
        .await?;

        if let Some(last_created) = last_action.created_at {
            let age = Utc::now() - last_created;
            if age < Duration::seconds(policy.dedup_window_seconds as i64) {
                let message = format!(
                    "Suppressed by dedup window ({}s)",
                    policy.dedup_window_seconds
                );
                insert_alert_policy_action(
                    &state.db,
                    policy.id,
                    alert.alert_id,
                    POLICY_ACTION_SUPPRESSED,
                    None,
                    Some(message.clone()),
                    json!({ "policy_key": policy.policy_key }),
                )
                .await?;

                write_audit_log_best_effort(
                    &state.db,
                    AuditLogWriteInput {
                        actor: actor.to_string(),
                        action: "alert.policy.ticket.suppressed".to_string(),
                        target_type: "alert_policy".to_string(),
                        target_id: Some(policy.id.to_string()),
                        result: "success".to_string(),
                        message: Some(message),
                        metadata: json!({
                            "alert_id": alert.alert_id,
                            "policy_key": policy.policy_key,
                        }),
                    },
                )
                .await;
                continue;
            }
        }

        match create_ticket_from_alert(&state.db, &policy, &alert).await {
            Ok(ticket_id) => {
                insert_alert_policy_action(
                    &state.db,
                    policy.id,
                    alert.alert_id,
                    POLICY_ACTION_TICKET_CREATED,
                    Some(ticket_id),
                    Some("Ticket created by alert policy".to_string()),
                    json!({
                        "policy_key": policy.policy_key,
                        "ticket_id": ticket_id,
                    }),
                )
                .await?;

                insert_alert_timeline(
                    &state.db,
                    alert.alert_id,
                    "ticket_created",
                    actor,
                    Some(format!(
                        "Auto-created ticket {ticket_id} via policy {}",
                        policy.policy_key
                    )),
                    json!({
                        "policy_id": policy.id,
                        "policy_key": policy.policy_key,
                        "ticket_id": ticket_id,
                    }),
                )
                .await?;

                write_audit_log_best_effort(
                    &state.db,
                    AuditLogWriteInput {
                        actor: actor.to_string(),
                        action: "alert.policy.ticket.create".to_string(),
                        target_type: "alert_policy".to_string(),
                        target_id: Some(policy.id.to_string()),
                        result: "success".to_string(),
                        message: None,
                        metadata: json!({
                            "alert_id": alert.alert_id,
                            "policy_key": policy.policy_key,
                            "ticket_id": ticket_id,
                        }),
                    },
                )
                .await;
            }
            Err(err) => {
                warn!(error = %err, policy_id = policy.id, alert_id = alert.alert_id, "failed to auto create ticket from alert policy");
                write_audit_log_best_effort(
                    &state.db,
                    AuditLogWriteInput {
                        actor: actor.to_string(),
                        action: "alert.policy.ticket.create".to_string(),
                        target_type: "alert_policy".to_string(),
                        target_id: Some(policy.id.to_string()),
                        result: "failed".to_string(),
                        message: Some(err.to_string()),
                        metadata: json!({
                            "alert_id": alert.alert_id,
                            "policy_key": policy.policy_key,
                        }),
                    },
                )
                .await;
            }
        }
    }

    Ok(())
}

fn policy_matches_alert(policy: &AlertTicketPolicyRecord, alert: &AlertPolicyMatchContext) -> bool {
    if let Some(source) = policy.match_source.as_deref() {
        if source != alert.alert_source {
            return false;
        }
    }
    if let Some(severity) = policy.match_severity.as_deref() {
        if severity != alert.severity {
            return false;
        }
    }
    if let Some(status) = policy.match_status.as_deref() {
        if status != alert.status {
            return false;
        }
    }
    if let Some(site) = policy.match_site.as_deref() {
        if alert.site.as_deref() != Some(site) {
            return false;
        }
    }
    if let Some(department) = policy.match_department.as_deref() {
        if alert.department.as_deref() != Some(department) {
            return false;
        }
    }
    true
}

async fn create_ticket_from_alert(
    db: &sqlx::PgPool,
    policy: &AlertTicketPolicyRecord,
    alert: &AlertPolicyMatchContext,
) -> AppResult<i64> {
    let mut tx = db.begin().await?;

    if let Some(template_id) = policy.workflow_template_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1
                FROM workflow_templates
                WHERE id = $1
            )",
        )
        .bind(template_id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(AppError::Validation(format!(
                "workflow_template_id {} does not exist",
                template_id
            )));
        }
    }

    let seed: TicketSeedRow = sqlx::query_as("SELECT nextval('tickets_id_seq') AS id")
        .fetch_one(&mut *tx)
        .await?;
    let ticket_id = seed.id;
    let ticket_no = format!("TKT-{}-{ticket_id:06}", Utc::now().format("%Y%m%d"));

    let title = format!("[Alert] {}", alert.title);
    let description = format!(
        "Auto-created by alert policy. source={}, key={}, severity={}, alert_id={}",
        alert.alert_source, alert.alert_key, alert.severity, alert.alert_id
    );
    let requester = format!("alert-policy:{}", policy.policy_key);

    sqlx::query(
        "INSERT INTO tickets (
             id, ticket_no, title, description, status, priority, category, requester, workflow_template_id, metadata
         )
         VALUES ($1, $2, $3, $4, 'open', $5, $6, $7, $8, $9)",
    )
    .bind(ticket_id)
    .bind(ticket_no)
    .bind(title)
    .bind(description)
    .bind(&policy.ticket_priority)
    .bind(&policy.ticket_category)
    .bind(requester)
    .bind(policy.workflow_template_id)
    .bind(json!({
        "alert_id": alert.alert_id,
        "alert_source": alert.alert_source,
        "alert_key": alert.alert_key,
        "policy_id": policy.id,
        "policy_key": policy.policy_key,
    }))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO ticket_alert_links (ticket_id, alert_source, alert_key, alert_title, severity)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT DO NOTHING",
    )
    .bind(ticket_id)
    .bind(&alert.alert_source)
    .bind(&alert.alert_key)
    .bind(&alert.title)
    .bind(&alert.severity)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ticket_id)
}

async fn insert_alert_policy_action(
    db: &sqlx::PgPool,
    policy_id: i64,
    alert_id: i64,
    action: &str,
    ticket_id: Option<i64>,
    message: Option<String>,
    metadata: Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO alert_policy_actions (policy_id, alert_id, action, ticket_id, message, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(policy_id)
    .bind(alert_id)
    .bind(action)
    .bind(ticket_id)
    .bind(message)
    .bind(metadata)
    .execute(db)
    .await?;
    Ok(())
}

async fn load_monitoring_asset_context(
    db: &sqlx::PgPool,
    asset_id: i64,
) -> AppResult<MonitoringAlertAssetContext> {
    let record: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT name, site, department
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(db)
    .await?;

    if let Some((name, site, department)) = record {
        Ok(MonitoringAlertAssetContext {
            name,
            site,
            department,
        })
    } else {
        Ok(MonitoringAlertAssetContext {
            name: format!("asset-{asset_id}"),
            site: None,
            department: None,
        })
    }
}

fn append_alert_filters(
    builder: &mut QueryBuilder<Postgres>,
    status: Option<String>,
    severity: Option<String>,
    source: Option<String>,
    site: Option<String>,
    department: Option<String>,
    query_text: Option<String>,
) {
    if let Some(status) = status {
        builder.push(" AND a.status = ").push_bind(status);
    }
    if let Some(severity) = severity {
        builder.push(" AND a.severity = ").push_bind(severity);
    }
    if let Some(source) = source {
        builder.push(" AND a.alert_source = ").push_bind(source);
    }
    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }
    if let Some(query_text) = query_text {
        let pattern = format!("%{}%", query_text.to_lowercase());
        builder
            .push(" AND (")
            .push("LOWER(a.title) LIKE ")
            .push_bind(pattern.clone())
            .push(" OR LOWER(a.alert_source) LIKE ")
            .push_bind(pattern.clone())
            .push(" OR LOWER(a.alert_key) LIKE ")
            .push_bind(pattern)
            .push(")");
    }
}

fn normalize_optional_status_filter(status: Option<String>) -> AppResult<Option<String>> {
    let Some(status) = status else {
        return Ok(None);
    };
    normalize_required_status(Some(status)).map(Some)
}

fn normalize_required_status(status: Option<String>) -> AppResult<String> {
    let Some(status) = status else {
        return Err(AppError::Validation("status is required".to_string()));
    };

    let normalized = status.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("status is required".to_string()));
    }

    match normalized.as_str() {
        ALERT_STATUS_OPEN | ALERT_STATUS_ACK | ALERT_STATUS_CLOSED => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "status must be one of: {ALERT_STATUS_OPEN}, {ALERT_STATUS_ACK}, {ALERT_STATUS_CLOSED}"
        ))),
    }
}

fn normalize_optional_severity_filter(severity: Option<String>) -> AppResult<Option<String>> {
    let Some(severity) = severity else {
        return Ok(None);
    };
    normalize_required_severity(Some(severity)).map(Some)
}

fn normalize_required_severity(severity: Option<String>) -> AppResult<String> {
    let Some(severity) = severity else {
        return Err(AppError::Validation("severity is required".to_string()));
    };

    let normalized = severity.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("severity is required".to_string()));
    }

    match normalized.as_str() {
        ALERT_SEVERITY_CRITICAL | ALERT_SEVERITY_WARNING | ALERT_SEVERITY_INFO => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "severity must be one of: {ALERT_SEVERITY_CRITICAL}, {ALERT_SEVERITY_WARNING}, {ALERT_SEVERITY_INFO}"
        ))),
    }
}

fn normalize_priority(priority: Option<String>) -> AppResult<String> {
    let normalized = priority
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "high".to_string());

    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => Ok(normalized),
        _ => Err(AppError::Validation(
            "ticket_priority must be one of: low, medium, high, critical".to_string(),
        )),
    }
}

fn normalize_alert_ids(ids: Vec<i64>) -> AppResult<Vec<i64>> {
    if ids.is_empty() {
        return Err(AppError::Validation("ids cannot be empty".to_string()));
    }

    let mut out = Vec::new();
    for id in ids {
        if id <= 0 {
            return Err(AppError::Validation("ids must be positive".to_string()));
        }
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

fn normalize_optional_positive_id(id: Option<i64>, field: &str) -> AppResult<Option<i64>> {
    match id {
        Some(value) if value <= 0 => Err(AppError::Validation(format!("{field} must be positive"))),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn validate_dedup_window(seconds: i32) -> AppResult<()> {
    if (30..=604_800).contains(&seconds) {
        Ok(())
    } else {
        Err(AppError::Validation(
            "dedup_window_seconds must be between 30 and 604800".to_string(),
        ))
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

fn truncate_message(input: &str, max_len: usize) -> &str {
    if input.len() > max_len {
        &input[..max_len]
    } else {
        input
    }
}

async fn resolve_actor(state: &AppState, headers: &HeaderMap) -> String {
    match resolve_auth_user(state, headers).await {
        Ok(actor) => actor,
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ALERT_SEVERITY_CRITICAL, ALERT_SEVERITY_INFO, ALERT_SEVERITY_WARNING, ALERT_STATUS_ACK,
        ALERT_STATUS_CLOSED, ALERT_STATUS_OPEN, normalize_optional_severity_filter,
        normalize_optional_status_filter, normalize_priority, validate_dedup_window,
    };

    #[test]
    fn validates_alert_status_values() {
        assert_eq!(
            normalize_optional_status_filter(Some(ALERT_STATUS_OPEN.to_string()))
                .expect("status should validate"),
            Some(ALERT_STATUS_OPEN.to_string())
        );
        assert_eq!(
            normalize_optional_status_filter(Some(ALERT_STATUS_ACK.to_string()))
                .expect("status should validate"),
            Some(ALERT_STATUS_ACK.to_string())
        );
        assert_eq!(
            normalize_optional_status_filter(Some(ALERT_STATUS_CLOSED.to_string()))
                .expect("status should validate"),
            Some(ALERT_STATUS_CLOSED.to_string())
        );
        assert!(normalize_optional_status_filter(Some("invalid".to_string())).is_err());
    }

    #[test]
    fn validates_alert_severity_values() {
        assert_eq!(
            normalize_optional_severity_filter(Some(ALERT_SEVERITY_CRITICAL.to_string()))
                .expect("severity should validate"),
            Some(ALERT_SEVERITY_CRITICAL.to_string())
        );
        assert_eq!(
            normalize_optional_severity_filter(Some(ALERT_SEVERITY_WARNING.to_string()))
                .expect("severity should validate"),
            Some(ALERT_SEVERITY_WARNING.to_string())
        );
        assert_eq!(
            normalize_optional_severity_filter(Some(ALERT_SEVERITY_INFO.to_string()))
                .expect("severity should validate"),
            Some(ALERT_SEVERITY_INFO.to_string())
        );
        assert!(normalize_optional_severity_filter(Some("broken".to_string())).is_err());
    }

    #[test]
    fn validates_ticket_priority_values() {
        assert_eq!(
            normalize_priority(Some("critical".to_string())).expect("priority should validate"),
            "critical"
        );
        assert!(normalize_priority(Some("urgent".to_string())).is_err());
    }

    #[test]
    fn validates_dedup_window_range() {
        assert!(validate_dedup_window(30).is_ok());
        assert!(validate_dedup_window(1800).is_ok());
        assert!(validate_dedup_window(604800).is_ok());
        assert!(validate_dedup_window(29).is_err());
        assert!(validate_dedup_window(604801).is_err());
    }
}
