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

const SYNC_SOURCE_ZABBIX: &str = "zabbix";
const SYNC_STATUS_PENDING: &str = "pending";
const DEFAULT_MAX_ATTEMPTS: i32 = 5;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/assets/{asset_id}/monitoring-binding",
            get(get_asset_monitoring_binding),
        )
        .route(
            "/assets/{asset_id}/monitoring-sync",
            axum::routing::post(trigger_asset_monitoring_sync),
        )
        .route("/monitoring-sync/jobs", get(list_monitoring_sync_jobs))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MonitoringBindingRecord {
    id: i64,
    asset_id: i64,
    source_system: String,
    source_id: Option<i64>,
    external_host_id: Option<String>,
    last_sync_status: String,
    last_sync_message: Option<String>,
    last_sync_at: Option<DateTime<Utc>>,
    mapping: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MonitoringSyncJobRecord {
    id: i64,
    asset_id: i64,
    trigger_source: String,
    status: String,
    attempt: i32,
    max_attempts: i32,
    run_after: DateTime<Utc>,
    requested_by: Option<String>,
    requested_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    payload: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct TriggerMonitoringSyncRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListMonitoringSyncJobsQuery {
    asset_id: Option<i64>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct AssetMonitoringBindingResponse {
    asset_id: i64,
    binding: Option<MonitoringBindingRecord>,
    latest_job: Option<MonitoringSyncJobRecord>,
}

#[derive(Debug, Serialize)]
struct TriggerMonitoringSyncResponse {
    asset_id: i64,
    job_id: i64,
    status: String,
}

#[derive(Debug, Serialize)]
struct ListMonitoringSyncJobsResponse {
    items: Vec<MonitoringSyncJobRecord>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
struct InflightJob {
    id: i64,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct AssetClassRow {
    asset_class: String,
}

async fn get_asset_monitoring_binding(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
) -> AppResult<Json<AssetMonitoringBindingResponse>> {
    ensure_asset_exists(&state.db, asset_id).await?;

    let binding: Option<MonitoringBindingRecord> = sqlx::query_as(
        "SELECT id, asset_id, source_system, source_id, external_host_id, last_sync_status, last_sync_message, last_sync_at, mapping, created_at, updated_at
         FROM cmdb_monitoring_bindings
         WHERE asset_id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    let latest_job: Option<MonitoringSyncJobRecord> = sqlx::query_as(
        "SELECT id, asset_id, trigger_source, status, attempt, max_attempts, run_after, requested_by, requested_at, started_at, completed_at, last_error, payload, created_at, updated_at
         FROM cmdb_monitoring_sync_jobs
         WHERE asset_id = $1
         ORDER BY requested_at DESC, id DESC
         LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(AssetMonitoringBindingResponse {
        asset_id,
        binding,
        latest_job,
    }))
}

async fn trigger_asset_monitoring_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(asset_id): Path<i64>,
    Json(payload): Json<TriggerMonitoringSyncRequest>,
) -> AppResult<Json<TriggerMonitoringSyncResponse>> {
    let requested_by = actor_from_headers(&headers);
    let reason = trim_optional(payload.reason);
    let job_id = enqueue_monitoring_sync_job(
        &state.db,
        asset_id,
        "manual_retry",
        requested_by.as_deref(),
        json!({ "reason": reason }),
    )
    .await?
    .ok_or_else(|| {
        AppError::Validation("asset class is not eligible for Zabbix auto-provisioning".to_string())
    })?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.monitoring_sync.trigger",
        "asset",
        Some(asset_id.to_string()),
        "success",
        None,
        json!({
            "job_id": job_id,
            "trigger_source": "manual_retry",
            "reason": reason
        }),
    )
    .await;

    Ok(Json(TriggerMonitoringSyncResponse {
        asset_id,
        job_id,
        status: "queued".to_string(),
    }))
}

async fn list_monitoring_sync_jobs(
    State(state): State<AppState>,
    Query(query): Query<ListMonitoringSyncJobsQuery>,
) -> AppResult<Json<ListMonitoringSyncJobsResponse>> {
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let status = trim_optional(query.status);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM cmdb_monitoring_sync_jobs WHERE 1=1");
    append_jobs_filters(&mut count_builder, query.asset_id, status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, asset_id, trigger_source, status, attempt, max_attempts, run_after, requested_by, requested_at, started_at, completed_at, last_error, payload, created_at, updated_at
         FROM cmdb_monitoring_sync_jobs
         WHERE 1=1",
    );
    append_jobs_filters(&mut list_builder, query.asset_id, status);
    list_builder
        .push(" ORDER BY requested_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<MonitoringSyncJobRecord> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListMonitoringSyncJobsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

fn append_jobs_filters(
    builder: &mut QueryBuilder<Postgres>,
    asset_id: Option<i64>,
    status: Option<String>,
) {
    if let Some(asset_id) = asset_id {
        builder.push(" AND asset_id = ").push_bind(asset_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ").push_bind(status);
    }
}

pub async fn enqueue_monitoring_sync_job(
    db: &sqlx::PgPool,
    asset_id: i64,
    trigger_source: &str,
    requested_by: Option<&str>,
    payload: Value,
) -> AppResult<Option<i64>> {
    let asset_class = get_asset_class(db, asset_id).await?;
    if !is_eligible_asset_class(asset_class.asset_class.as_str()) {
        return Ok(None);
    }

    let payload = normalize_payload(payload)?;
    let trigger_source = normalize_trigger_source(trigger_source)?;
    let requested_by = trim_optional(requested_by.map(ToString::to_string));

    let inflight: Option<InflightJob> = sqlx::query_as(
        "SELECT id, status
         FROM cmdb_monitoring_sync_jobs
         WHERE asset_id = $1
           AND status IN ('pending', 'running')
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(asset_id)
    .fetch_optional(db)
    .await?;

    if let Some(inflight) = inflight {
        if inflight.status == SYNC_STATUS_PENDING {
            sqlx::query(
                "UPDATE cmdb_monitoring_sync_jobs
                 SET run_after = NOW(),
                     requested_by = COALESCE($2, requested_by),
                     payload = $3,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(inflight.id)
            .bind(requested_by)
            .bind(payload)
            .execute(db)
            .await?;
        }
        return Ok(Some(inflight.id));
    }

    let job_id: i64 = sqlx::query_scalar(
        "INSERT INTO cmdb_monitoring_sync_jobs (asset_id, trigger_source, status, attempt, max_attempts, run_after, requested_by, payload)
         VALUES ($1, $2, 'pending', 0, $3, NOW(), $4, $5)
         RETURNING id",
    )
    .bind(asset_id)
    .bind(trigger_source)
    .bind(DEFAULT_MAX_ATTEMPTS)
    .bind(requested_by)
    .bind(payload)
    .fetch_one(db)
    .await?;

    sqlx::query(
        "INSERT INTO cmdb_monitoring_bindings (asset_id, source_system, last_sync_status, last_sync_message, last_sync_at, mapping)
         VALUES ($1, $2, 'pending', $3, NOW(), '{}'::jsonb)
         ON CONFLICT (asset_id) DO UPDATE
         SET last_sync_status = 'pending',
             last_sync_message = EXCLUDED.last_sync_message,
             last_sync_at = NOW(),
             updated_at = NOW()",
    )
    .bind(asset_id)
    .bind(SYNC_SOURCE_ZABBIX)
    .bind(format!("monitoring sync job queued by {}", trigger_source))
    .execute(db)
    .await?;

    Ok(Some(job_id))
}

pub fn is_eligible_asset_class(asset_class: &str) -> bool {
    matches!(
        asset_class.trim().to_ascii_lowercase().as_str(),
        "server" | "virtual_machine" | "vm" | "network_device" | "container" | "database"
    )
}

async fn get_asset_class(db: &sqlx::PgPool, asset_id: i64) -> AppResult<AssetClassRow> {
    let item: Option<AssetClassRow> =
        sqlx::query_as("SELECT asset_class FROM assets WHERE id = $1")
            .bind(asset_id)
            .fetch_optional(db)
            .await?;
    item.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))
}

async fn ensure_asset_exists(db: &sqlx::PgPool, asset_id: i64) -> AppResult<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM assets WHERE id = $1")
        .bind(asset_id)
        .fetch_optional(db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("asset {asset_id} not found")));
    }
    Ok(())
}

fn normalize_payload(payload: Value) -> AppResult<Value> {
    if payload.is_object() {
        Ok(payload)
    } else {
        Err(AppError::Validation(
            "payload must be a JSON object".to_string(),
        ))
    }
}

fn normalize_trigger_source(value: &str) -> AppResult<&str> {
    match value {
        "asset_create" | "asset_update" | "manual_retry" => Ok(value),
        _ => Err(AppError::Validation(
            "trigger_source must be one of: asset_create, asset_update, manual_retry".to_string(),
        )),
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
