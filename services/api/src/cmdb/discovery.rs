use std::{collections::HashSet, time::Duration};

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::{Postgres, QueryBuilder};
use tokio::time::sleep;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

const SOURCE_ZABBIX_HOSTS: &str = "zabbix_hosts";
const SOURCE_SNMP_SEED: &str = "snmp_seed";
const SOURCE_K8S_SEED: &str = "k8s_seed";

const EVENT_ASSET_NEW_DETECTED: &str = "asset.new_detected";
const EVENT_ASSET_OFFBOARDED_SUSPECTED: &str = "asset.offboarded_suspected";
const EVENT_ASSET_PROFILE_CHANGED: &str = "asset.profile_changed";

const DELIVERY_STATUS_QUEUED: &str = "queued";
const DELIVERY_STATUS_DELIVERED: &str = "delivered";
const DELIVERY_STATUS_FAILED: &str = "failed";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/discovery/jobs",
            get(list_discovery_jobs).post(create_discovery_job),
        )
        .route(
            "/discovery/jobs/{job_id}/run",
            axum::routing::post(run_discovery_job),
        )
        .route("/discovery/candidates", get(list_discovery_candidates))
        .route(
            "/discovery/candidates/{candidate_id}/approve",
            axum::routing::post(approve_discovery_candidate),
        )
        .route(
            "/discovery/candidates/{candidate_id}/reject",
            axum::routing::post(reject_discovery_candidate),
        )
        .route("/discovery/events", get(list_discovery_events))
        .route(
            "/discovery/notification-deliveries",
            get(list_notification_deliveries),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct DiscoveryJob {
    id: i64,
    name: String,
    source_type: String,
    scope: Value,
    schedule: Option<String>,
    status: String,
    is_enabled: bool,
    last_run_at: Option<DateTime<Utc>>,
    last_run_status: Option<String>,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct DiscoveryCandidate {
    id: i64,
    job_id: Option<i64>,
    fingerprint: String,
    payload: Value,
    review_status: String,
    discovered_at: DateTime<Utc>,
    reviewed_by: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct DiscoveryEvent {
    id: i64,
    job_id: Option<i64>,
    asset_id: Option<i64>,
    event_type: String,
    fingerprint: Option<String>,
    payload: Value,
    happened_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct DiscoveryAssetState {
    id: i64,
    asset_id: i64,
    profile: Value,
    last_seen_at: Option<DateTime<Utc>>,
    missed_runs: i32,
    offboarded_emitted: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NotificationDelivery {
    id: i64,
    event_id: i64,
    subscription_id: Option<i64>,
    channel_id: Option<i64>,
    target: String,
    status: String,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
    delivered_at: Option<DateTime<Utc>>,
    payload: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NotificationDispatchTarget {
    subscription_id: i64,
    channel_id: i64,
    channel_type: String,
    target: String,
    config: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct NotificationTemplateRecord {
    title_template: String,
    body_template: String,
}

#[derive(Debug, Deserialize)]
struct CreateDiscoveryJobRequest {
    name: String,
    source_type: String,
    scope: Option<Value>,
    schedule: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListDiscoveryCandidatesQuery {
    review_status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ListDiscoveryEventsQuery {
    job_id: Option<i64>,
    asset_id: Option<i64>,
    event_type: Option<String>,
    time_from: Option<String>,
    time_to: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ListNotificationDeliveriesQuery {
    event_id: Option<i64>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ReviewDiscoveryCandidateRequest {
    reviewed_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListDiscoveryCandidatesResponse {
    items: Vec<DiscoveryCandidate>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct ListDiscoveryEventsResponse {
    items: Vec<DiscoveryEvent>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct ListNotificationDeliveriesResponse {
    items: Vec<NotificationDelivery>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct ReviewDiscoveryCandidateResponse {
    candidate: DiscoveryCandidate,
    action: String,
    asset_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct RunDiscoveryJobResponse {
    job: DiscoveryJob,
    stats: DiscoveryRunStats,
}

#[derive(Debug, Serialize, Default)]
struct DiscoveryRunStats {
    matched_assets: u32,
    queued_candidates: u32,
    skipped_candidates: u32,
    new_detected_events: u32,
    profile_changed_events: u32,
    offboarded_suspected_events: u32,
}

#[derive(Debug)]
struct DiscoveryInputItem {
    name: String,
    hostname: Option<String>,
    ip: Option<String>,
    asset_class: String,
    resource_kind: String,
    metadata: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct AssetMatch {
    id: i64,
    name: String,
}

async fn create_discovery_job(
    State(state): State<AppState>,
    Json(payload): Json<CreateDiscoveryJobRequest>,
) -> AppResult<Json<DiscoveryJob>> {
    let name = required_trimmed("name", payload.name)?;
    let source_type = normalize_source_type(payload.source_type)?;
    let scope = normalize_scope(payload.scope)?;
    let schedule = trim_optional(payload.schedule);
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let job: DiscoveryJob = sqlx::query_as(
        "INSERT INTO discovery_jobs (name, source_type, scope, schedule, status, is_enabled)
         VALUES ($1, $2, $3, $4, 'idle', $5)
         RETURNING id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at",
    )
    .bind(name)
    .bind(source_type)
    .bind(scope)
    .bind(schedule)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(job))
}

async fn list_discovery_jobs(State(state): State<AppState>) -> AppResult<Json<Vec<DiscoveryJob>>> {
    let jobs: Vec<DiscoveryJob> = sqlx::query_as(
        "SELECT id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at
         FROM discovery_jobs
         ORDER BY id DESC",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(jobs))
}

async fn run_discovery_job(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
) -> AppResult<Json<RunDiscoveryJobResponse>> {
    let existing: Option<DiscoveryJob> = sqlx::query_as(
        "SELECT id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at
         FROM discovery_jobs
         WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?;

    let existing =
        existing.ok_or_else(|| AppError::NotFound(format!("discovery job {job_id} not found")))?;

    if !existing.is_enabled {
        return Err(AppError::Validation(format!(
            "discovery job {job_id} is disabled"
        )));
    }

    let running_job = mark_discovery_job_running(&state.db, job_id).await?;

    let run_result = execute_discovery_job(&state.db, &running_job).await;
    match run_result {
        Ok(stats) => {
            let final_job = mark_discovery_job_success(&state.db, job_id).await?;
            Ok(Json(RunDiscoveryJobResponse {
                job: final_job,
                stats,
            }))
        }
        Err(err) => {
            let message = err.to_string();
            let _ = mark_discovery_job_failed(&state.db, job_id, &message).await;
            Err(err)
        }
    }
}

async fn list_discovery_candidates(
    State(state): State<AppState>,
    Query(query): Query<ListDiscoveryCandidatesQuery>,
) -> AppResult<Json<ListDiscoveryCandidatesResponse>> {
    let limit = query.limit.unwrap_or(20).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let review_status = trim_optional(query.review_status);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM discovery_candidates WHERE 1=1");
    append_candidate_filters(&mut count_builder, review_status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, job_id, fingerprint, payload, review_status, discovered_at, reviewed_by, reviewed_at, created_at, updated_at
         FROM discovery_candidates
         WHERE 1=1",
    );
    append_candidate_filters(&mut list_builder, review_status);
    list_builder
        .push(" ORDER BY discovered_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<DiscoveryCandidate> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListDiscoveryCandidatesResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn approve_discovery_candidate(
    State(state): State<AppState>,
    Path(candidate_id): Path<i64>,
    Json(payload): Json<ReviewDiscoveryCandidateRequest>,
) -> AppResult<Json<ReviewDiscoveryCandidateResponse>> {
    let reviewed_by = normalize_reviewer(payload.reviewed_by);
    let candidate = get_candidate_by_id(&state.db, candidate_id).await?;
    ensure_candidate_pending(&candidate)?;

    let candidate_payload = candidate
        .payload
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::Validation("candidate payload must be object".to_string()))?;

    let name = candidate_payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("candidate payload missing name".to_string()))?
        .to_string();
    let asset_class = candidate_payload
        .get("asset_class")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("server")
        .to_string();
    let hostname = candidate_payload
        .get("hostname")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let ip = candidate_payload
        .get("ip")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let asset_match = find_asset_by_hostname_ip(&state.db, &hostname, &ip).await?;
    let (action, asset_id) = if let Some(asset_match) = asset_match {
        ("merged".to_string(), Some(asset_match.id))
    } else {
        let metadata = candidate_payload
            .get("metadata")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let mut custom_fields = metadata;
        if let Some(value) = candidate_payload
            .get("resource_kind")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            custom_fields.insert("resource_kind".to_string(), Value::String(value));
        }
        if let Some(value) = candidate_payload
            .get("source_type")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            custom_fields.insert("source_type".to_string(), Value::String(value));
        }

        let site = candidate_payload
            .get("site")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let department = candidate_payload
            .get("department")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let owner = candidate_payload
            .get("owner")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        let created_asset_id: i64 = sqlx::query_scalar(
            "INSERT INTO assets (asset_class, name, hostname, ip, status, site, department, owner, custom_fields)
             VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8)
             RETURNING id",
        )
        .bind(asset_class)
        .bind(name)
        .bind(hostname)
        .bind(ip)
        .bind(site)
        .bind(department)
        .bind(owner)
        .bind(Value::Object(custom_fields))
        .fetch_one(&state.db)
        .await?;

        ("created".to_string(), Some(created_asset_id))
    };

    let reviewed_candidate: DiscoveryCandidate = sqlx::query_as(
        "UPDATE discovery_candidates
         SET review_status = $2,
             reviewed_by = $3,
             reviewed_at = NOW(),
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, job_id, fingerprint, payload, review_status, discovered_at, reviewed_by, reviewed_at, created_at, updated_at",
    )
    .bind(candidate_id)
    .bind(action.as_str())
    .bind(reviewed_by)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ReviewDiscoveryCandidateResponse {
        candidate: reviewed_candidate,
        action,
        asset_id,
    }))
}

async fn reject_discovery_candidate(
    State(state): State<AppState>,
    Path(candidate_id): Path<i64>,
    Json(payload): Json<ReviewDiscoveryCandidateRequest>,
) -> AppResult<Json<ReviewDiscoveryCandidateResponse>> {
    let reviewed_by = normalize_reviewer(payload.reviewed_by);
    let candidate = get_candidate_by_id(&state.db, candidate_id).await?;
    ensure_candidate_pending(&candidate)?;

    let reviewed_candidate: DiscoveryCandidate = sqlx::query_as(
        "UPDATE discovery_candidates
         SET review_status = 'rejected',
             reviewed_by = $2,
             reviewed_at = NOW(),
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, job_id, fingerprint, payload, review_status, discovered_at, reviewed_by, reviewed_at, created_at, updated_at",
    )
    .bind(candidate_id)
    .bind(reviewed_by)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ReviewDiscoveryCandidateResponse {
        candidate: reviewed_candidate,
        action: "rejected".to_string(),
        asset_id: None,
    }))
}

async fn list_discovery_events(
    State(state): State<AppState>,
    Query(query): Query<ListDiscoveryEventsQuery>,
) -> AppResult<Json<ListDiscoveryEventsResponse>> {
    let limit = query.limit.unwrap_or(50).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let event_type = trim_optional(query.event_type).map(|item| item.to_ascii_lowercase());
    let time_from = parse_time_filter(query.time_from, "time_from")?;
    let time_to = parse_time_filter(query.time_to, "time_to")?;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM discovery_events WHERE 1=1");
    append_event_filters(
        &mut count_builder,
        query.job_id,
        query.asset_id,
        event_type.clone(),
        time_from,
        time_to,
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, job_id, asset_id, event_type, fingerprint, payload, happened_at, created_at
         FROM discovery_events
         WHERE 1=1",
    );
    append_event_filters(
        &mut list_builder,
        query.job_id,
        query.asset_id,
        event_type,
        time_from,
        time_to,
    );
    list_builder
        .push(" ORDER BY happened_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<DiscoveryEvent> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListDiscoveryEventsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn list_notification_deliveries(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationDeliveriesQuery>,
) -> AppResult<Json<ListNotificationDeliveriesResponse>> {
    let limit = query.limit.unwrap_or(50).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let status = trim_optional(query.status).map(|value| value.to_ascii_lowercase());

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM discovery_notification_deliveries WHERE 1=1");
    append_delivery_filters(&mut count_builder, query.event_id, status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, event_id, subscription_id, channel_id, target, status, attempts, response_code, last_error, delivered_at, payload, created_at, updated_at
         FROM discovery_notification_deliveries
         WHERE 1=1",
    );
    append_delivery_filters(&mut list_builder, query.event_id, status);
    list_builder
        .push(" ORDER BY created_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<NotificationDelivery> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListNotificationDeliveriesResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn execute_discovery_job(
    db: &sqlx::PgPool,
    job: &DiscoveryJob,
) -> AppResult<DiscoveryRunStats> {
    let items = collect_discovery_items(job).await?;
    let offboarded_threshold = parse_offboarded_threshold(&job.scope);

    let mut stats = DiscoveryRunStats::default();
    let mut seen_asset_ids = HashSet::new();

    for item in items {
        let fingerprint = build_fingerprint(&item);
        let candidate_payload = build_candidate_payload(job.source_type.as_str(), &item);

        if let Some(asset_match) = find_asset_by_hostname_ip(db, &item.hostname, &item.ip).await? {
            stats.matched_assets += 1;
            seen_asset_ids.insert(asset_match.id);

            let profile = build_asset_profile(&item);
            let changed = apply_asset_state_for_match(
                db,
                job.id,
                asset_match.id,
                &profile,
                job.source_type.as_str(),
                &fingerprint,
                &asset_match.name,
            )
            .await?;
            if changed {
                stats.profile_changed_events += 1;
            }
            continue;
        }

        if pending_candidate_exists(db, &fingerprint).await? {
            stats.skipped_candidates += 1;
            continue;
        }

        sqlx::query(
            "INSERT INTO discovery_candidates (job_id, fingerprint, payload, review_status)
             VALUES ($1, $2, $3, 'pending')",
        )
        .bind(job.id)
        .bind(&fingerprint)
        .bind(candidate_payload.clone())
        .execute(db)
        .await?;

        stats.queued_candidates += 1;

        emit_discovery_event(
            db,
            job.id,
            None,
            EVENT_ASSET_NEW_DETECTED,
            Some(&fingerprint),
            &candidate_payload,
        )
        .await?;
        stats.new_detected_events += 1;
    }

    let offboarded_events = process_missing_assets(
        db,
        job.id,
        &seen_asset_ids,
        offboarded_threshold,
        job.source_type.as_str(),
    )
    .await?;
    stats.offboarded_suspected_events = offboarded_events as u32;

    Ok(stats)
}

async fn collect_discovery_items(job: &DiscoveryJob) -> AppResult<Vec<DiscoveryInputItem>> {
    match job.source_type.as_str() {
        SOURCE_ZABBIX_HOSTS => collect_from_zabbix_hosts(&job.scope).await,
        SOURCE_SNMP_SEED => collect_from_snmp_seed(&job.scope),
        SOURCE_K8S_SEED => collect_from_k8s_seed(&job.scope),
        _ => Err(AppError::Validation(format!(
            "unsupported source_type '{}', supported: {}",
            job.source_type,
            supported_source_types().join(", ")
        ))),
    }
}

async fn collect_from_zabbix_hosts(scope: &Value) -> AppResult<Vec<DiscoveryInputItem>> {
    if let Some(hosts) = scope.get("mock_hosts").and_then(Value::as_array) {
        let mut items = Vec::with_capacity(hosts.len());
        for host in hosts {
            let object = host.as_object().ok_or_else(|| {
                AppError::Validation("scope.mock_hosts must contain JSON objects".to_string())
            })?;

            items.push(DiscoveryInputItem {
                name: pick_string(object, &["name", "host", "hostname"])?.to_string(),
                hostname: optional_string(object, &["hostname", "host"]),
                ip: optional_string(object, &["ip"]),
                asset_class: optional_string(object, &["asset_class"])
                    .unwrap_or_else(|| "server".to_string()),
                resource_kind: "host".to_string(),
                metadata: Value::Object(object.clone()),
            });
        }
        return Ok(items);
    }

    let scope_obj = scope
        .as_object()
        .ok_or_else(|| AppError::Validation("scope must be a JSON object".to_string()))?;

    let endpoint = pick_string(scope_obj, &["endpoint", "url"])?;
    let token = pick_string(scope_obj, &["token", "auth"])?;

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| AppError::Internal(anyhow!(err)))?;

    let request_body = json!({
        "jsonrpc": "2.0",
        "method": "host.get",
        "params": {
            "output": ["host", "name", "hostid"],
            "selectInterfaces": ["ip", "dns"]
        },
        "auth": token,
        "id": 1
    });

    let response = client
        .post(endpoint)
        .json(&request_body)
        .send()
        .await
        .map_err(|err| AppError::Internal(anyhow!(err)))?;

    if !response.status().is_success() {
        return Err(AppError::Validation(format!(
            "zabbix host pull failed with status {}",
            response.status()
        )));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|err| AppError::Internal(anyhow!(err)))?;

    if let Some(error) = payload.get("error") {
        return Err(AppError::Validation(format!(
            "zabbix API returned error: {error}"
        )));
    }

    let result = payload
        .get("result")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Validation("zabbix response.result must be an array".to_string())
        })?;

    let mut items = Vec::with_capacity(result.len());
    for host in result {
        let object = host.as_object().ok_or_else(|| {
            AppError::Validation("zabbix result entries must be objects".to_string())
        })?;

        let host_name =
            optional_string(object, &["name", "host"]).unwrap_or_else(|| "unknown".to_string());
        let hostname = optional_string(object, &["host"]);

        let ip = object
            .get("interfaces")
            .and_then(Value::as_array)
            .and_then(|interfaces| {
                interfaces.iter().find_map(|item| {
                    item.as_object().and_then(|it| {
                        it.get("ip")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToString::to_string)
                    })
                })
            });

        items.push(DiscoveryInputItem {
            name: host_name,
            hostname,
            ip,
            asset_class: "server".to_string(),
            resource_kind: "host".to_string(),
            metadata: Value::Object(object.clone()),
        });
    }

    Ok(items)
}

fn collect_from_snmp_seed(scope: &Value) -> AppResult<Vec<DiscoveryInputItem>> {
    let devices = scope
        .get("seed_devices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Validation(
                "snmp_seed discovery requires scope.seed_devices array in MVP".to_string(),
            )
        })?;

    let mut items = Vec::with_capacity(devices.len());
    for device in devices {
        let object = device.as_object().ok_or_else(|| {
            AppError::Validation("scope.seed_devices must contain JSON objects".to_string())
        })?;

        items.push(DiscoveryInputItem {
            name: pick_string(object, &["name", "hostname", "ip"])?.to_string(),
            hostname: optional_string(object, &["hostname", "name"]),
            ip: optional_string(object, &["ip"]),
            asset_class: optional_string(object, &["asset_class", "device_type"])
                .unwrap_or_else(|| "network_device".to_string()),
            resource_kind: "network_device".to_string(),
            metadata: Value::Object(object.clone()),
        });
    }

    Ok(items)
}

fn collect_from_k8s_seed(scope: &Value) -> AppResult<Vec<DiscoveryInputItem>> {
    let containers = scope
        .get("seed_containers")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Validation(
                "k8s_seed discovery requires scope.seed_containers array in MVP".to_string(),
            )
        })?;

    let mut items = Vec::with_capacity(containers.len());
    for container in containers {
        let object = container.as_object().ok_or_else(|| {
            AppError::Validation("scope.seed_containers must contain JSON objects".to_string())
        })?;

        let name = pick_string(object, &["container", "name", "pod"])?;
        items.push(DiscoveryInputItem {
            name: name.to_string(),
            hostname: None,
            ip: optional_string(object, &["ip", "pod_ip"]),
            asset_class: "container".to_string(),
            resource_kind: "container".to_string(),
            metadata: Value::Object(object.clone()),
        });
    }

    Ok(items)
}

async fn mark_discovery_job_running(db: &sqlx::PgPool, job_id: i64) -> AppResult<DiscoveryJob> {
    sqlx::query_as(
        "UPDATE discovery_jobs
         SET status = 'running',
             last_run_at = NOW(),
             last_run_status = 'running',
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at",
    )
    .bind(job_id)
    .fetch_one(db)
    .await
    .map_err(AppError::from)
}

async fn mark_discovery_job_success(db: &sqlx::PgPool, job_id: i64) -> AppResult<DiscoveryJob> {
    sqlx::query_as(
        "UPDATE discovery_jobs
         SET status = 'idle',
             last_run_status = 'success',
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at",
    )
    .bind(job_id)
    .fetch_one(db)
    .await
    .map_err(AppError::from)
}

async fn mark_discovery_job_failed(
    db: &sqlx::PgPool,
    job_id: i64,
    message: &str,
) -> AppResult<DiscoveryJob> {
    sqlx::query_as(
        "UPDATE discovery_jobs
         SET status = 'failed',
             last_run_status = 'failed',
             last_error = $2,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at",
    )
    .bind(job_id)
    .bind(truncate_error(message))
    .fetch_one(db)
    .await
    .map_err(AppError::from)
}

async fn find_asset_by_hostname_ip(
    db: &sqlx::PgPool,
    hostname: &Option<String>,
    ip: &Option<String>,
) -> AppResult<Option<AssetMatch>> {
    let (Some(hostname), Some(ip)) = (hostname.as_ref(), ip.as_ref()) else {
        return Ok(None);
    };

    let item: Option<AssetMatch> = sqlx::query_as(
        "SELECT id, name
         FROM assets
         WHERE LOWER(COALESCE(hostname, '')) = LOWER($1)
           AND COALESCE(ip, '') = $2
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(hostname)
    .bind(ip)
    .fetch_optional(db)
    .await?;

    Ok(item)
}

async fn apply_asset_state_for_match(
    db: &sqlx::PgPool,
    job_id: i64,
    asset_id: i64,
    profile: &Value,
    source_type: &str,
    fingerprint: &str,
    asset_name: &str,
) -> AppResult<bool> {
    let existing: Option<DiscoveryAssetState> = sqlx::query_as(
        "SELECT id, asset_id, profile, last_seen_at, missed_runs, offboarded_emitted
         FROM discovery_asset_states
         WHERE job_id = $1 AND asset_id = $2",
    )
    .bind(job_id)
    .bind(asset_id)
    .fetch_optional(db)
    .await?;

    let mut changed = false;
    if let Some(state) = existing {
        if state.profile != *profile {
            let payload = json!({
                "source_type": source_type,
                "asset_id": asset_id,
                "asset_name": asset_name,
                "before": state.profile,
                "after": profile,
            });
            emit_discovery_event(
                db,
                job_id,
                Some(asset_id),
                EVENT_ASSET_PROFILE_CHANGED,
                Some(fingerprint),
                &payload,
            )
            .await?;
            changed = true;
        }

        sqlx::query(
            "UPDATE discovery_asset_states
             SET profile = $3,
                 last_seen_at = NOW(),
                 missed_runs = 0,
                 offboarded_emitted = FALSE,
                 updated_at = NOW()
             WHERE id = $1 AND job_id = $2",
        )
        .bind(state.id)
        .bind(job_id)
        .bind(profile)
        .execute(db)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO discovery_asset_states (job_id, asset_id, profile, last_seen_at, missed_runs, offboarded_emitted)
             VALUES ($1, $2, $3, NOW(), 0, FALSE)",
        )
        .bind(job_id)
        .bind(asset_id)
        .bind(profile)
        .execute(db)
        .await?;
    }

    Ok(changed)
}

async fn process_missing_assets(
    db: &sqlx::PgPool,
    job_id: i64,
    seen_asset_ids: &HashSet<i64>,
    offboarded_threshold: i32,
    source_type: &str,
) -> AppResult<usize> {
    let states: Vec<DiscoveryAssetState> = sqlx::query_as(
        "SELECT id, asset_id, profile, last_seen_at, missed_runs, offboarded_emitted
         FROM discovery_asset_states
         WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_all(db)
    .await?;

    let mut emitted = 0_usize;

    for state in states {
        if seen_asset_ids.contains(&state.asset_id) {
            continue;
        }

        let next_missed = state.missed_runs.saturating_add(1);
        let mut offboarded_emitted = state.offboarded_emitted;

        if !offboarded_emitted && next_missed >= offboarded_threshold {
            let payload = json!({
                "source_type": source_type,
                "asset_id": state.asset_id,
                "missed_runs": next_missed,
                "offboarded_threshold": offboarded_threshold,
                "last_seen_at": state.last_seen_at,
            });
            emit_discovery_event(
                db,
                job_id,
                Some(state.asset_id),
                EVENT_ASSET_OFFBOARDED_SUSPECTED,
                None,
                &payload,
            )
            .await?;
            offboarded_emitted = true;
            emitted += 1;
        }

        sqlx::query(
            "UPDATE discovery_asset_states
             SET missed_runs = $2,
                 offboarded_emitted = $3,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(state.id)
        .bind(next_missed)
        .bind(offboarded_emitted)
        .execute(db)
        .await?;
    }

    Ok(emitted)
}

async fn emit_discovery_event(
    db: &sqlx::PgPool,
    job_id: i64,
    asset_id: Option<i64>,
    event_type: &str,
    fingerprint: Option<&str>,
    payload: &Value,
) -> AppResult<()> {
    let payload_obj = payload.as_object().cloned().unwrap_or_default();
    let payload_value = Value::Object(payload_obj.clone());

    let event_id: i64 = sqlx::query_scalar(
        "INSERT INTO discovery_events (job_id, asset_id, event_type, fingerprint, payload)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(job_id)
    .bind(asset_id)
    .bind(event_type)
    .bind(fingerprint)
    .bind(payload_value.clone())
    .fetch_one(db)
    .await?;

    dispatch_notifications_for_event(db, event_id, event_type, &payload_value).await?;

    Ok(())
}

async fn dispatch_notifications_for_event(
    db: &sqlx::PgPool,
    event_id: i64,
    event_type: &str,
    payload: &Value,
) -> AppResult<()> {
    let site = payload
        .get("site")
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let department = payload
        .get("department")
        .and_then(Value::as_str)
        .map(|v| v.to_string());

    let targets = load_notification_targets(db, event_type, site, department).await?;
    if targets.is_empty() {
        return Ok(());
    }

    let template = load_notification_template(db, event_type).await?;
    let payload_obj = payload.as_object().cloned().unwrap_or_default();

    for target in targets {
        let title_template = template
            .as_ref()
            .map(|item| item.title_template.as_str())
            .unwrap_or("CMDB Discovery Event");
        let body_template = template
            .as_ref()
            .map(|item| item.body_template.as_str())
            .unwrap_or("Event {{event_type}} occurred.");

        let mut render_payload = payload_obj.clone();
        render_payload.insert(
            "event_type".to_string(),
            Value::String(event_type.to_string()),
        );

        let title = render_template(title_template, &render_payload);
        let body = render_template(body_template, &render_payload);
        let message = json!({
            "event_id": event_id,
            "event_type": event_type,
            "title": title,
            "body": body,
            "payload": payload,
        });

        let delivery_id = create_delivery_record(
            db,
            event_id,
            target.subscription_id,
            target.channel_id,
            &target.target,
            &message,
        )
        .await?;

        let outcome = send_notification_to_target(&target, &message).await;
        finalize_delivery_record(
            db,
            delivery_id,
            outcome.status,
            outcome.attempts,
            outcome.response_code,
            outcome.last_error.as_deref(),
        )
        .await?;
    }

    Ok(())
}

struct DeliveryOutcome {
    status: &'static str,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
}

async fn send_notification_to_target(
    target: &NotificationDispatchTarget,
    message: &Value,
) -> DeliveryOutcome {
    match target.channel_type.as_str() {
        "webhook" => send_webhook_with_retry(target, message).await,
        "email" => DeliveryOutcome {
            status: DELIVERY_STATUS_DELIVERED,
            attempts: 1,
            response_code: Some(202),
            last_error: None,
        },
        _ => DeliveryOutcome {
            status: DELIVERY_STATUS_FAILED,
            attempts: 1,
            response_code: None,
            last_error: Some(format!(
                "unsupported channel_type '{}'",
                target.channel_type
            )),
        },
    }
}

async fn send_webhook_with_retry(
    target: &NotificationDispatchTarget,
    message: &Value,
) -> DeliveryOutcome {
    let max_attempts = target
        .config
        .get("max_attempts")
        .and_then(Value::as_i64)
        .map(|v| v.clamp(1, 5) as i32)
        .unwrap_or(3);
    let base_delay_ms = target
        .config
        .get("base_delay_ms")
        .and_then(Value::as_u64)
        .map(|v| v.clamp(50, 10_000))
        .unwrap_or(200);

    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(client) => client,
        Err(err) => {
            return DeliveryOutcome {
                status: DELIVERY_STATUS_FAILED,
                attempts: 1,
                response_code: None,
                last_error: Some(err.to_string()),
            };
        }
    };

    let mut attempts: i32 = 0;
    let mut last_error: Option<String> = None;
    let mut last_code: Option<i32> = None;

    while attempts < max_attempts {
        attempts += 1;
        let response = client.post(&target.target).json(message).send().await;
        match response {
            Ok(response) => {
                let code = response.status().as_u16() as i32;
                last_code = Some(code);
                if response.status().is_success() {
                    return DeliveryOutcome {
                        status: DELIVERY_STATUS_DELIVERED,
                        attempts,
                        response_code: Some(code),
                        last_error: None,
                    };
                }

                let body = response.text().await.unwrap_or_default();
                last_error = Some(format!("webhook responded with status {code}: {body}"));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        if attempts < max_attempts {
            let factor = 1_u64 << (attempts as u32 - 1);
            let wait = base_delay_ms.saturating_mul(factor);
            sleep(Duration::from_millis(wait)).await;
        }
    }

    DeliveryOutcome {
        status: DELIVERY_STATUS_FAILED,
        attempts,
        response_code: last_code,
        last_error,
    }
}

async fn load_notification_targets(
    db: &sqlx::PgPool,
    event_type: &str,
    site: Option<String>,
    department: Option<String>,
) -> AppResult<Vec<NotificationDispatchTarget>> {
    let items: Vec<NotificationDispatchTarget> = sqlx::query_as(
        "SELECT
            s.id AS subscription_id,
            c.id AS channel_id,
            c.channel_type,
            c.target,
            c.config
         FROM discovery_notification_subscriptions s
         INNER JOIN discovery_notification_channels c ON c.id = s.channel_id
         WHERE s.is_enabled = TRUE
           AND c.is_enabled = TRUE
           AND s.event_type = $1
           AND (s.site IS NULL OR s.site = $2)
           AND (s.department IS NULL OR s.department = $3)
         ORDER BY s.id ASC",
    )
    .bind(event_type)
    .bind(site)
    .bind(department)
    .fetch_all(db)
    .await?;

    Ok(items)
}

async fn load_notification_template(
    db: &sqlx::PgPool,
    event_type: &str,
) -> AppResult<Option<NotificationTemplateRecord>> {
    let item: Option<NotificationTemplateRecord> = sqlx::query_as(
        "SELECT title_template, body_template
         FROM discovery_notification_templates
         WHERE event_type = $1
           AND is_enabled = TRUE
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(event_type)
    .fetch_optional(db)
    .await?;

    Ok(item)
}

async fn create_delivery_record(
    db: &sqlx::PgPool,
    event_id: i64,
    subscription_id: i64,
    channel_id: i64,
    target: &str,
    payload: &Value,
) -> AppResult<i64> {
    let delivery_id: i64 = sqlx::query_scalar(
        "INSERT INTO discovery_notification_deliveries
            (event_id, subscription_id, channel_id, target, status, attempts, payload)
         VALUES ($1, $2, $3, $4, $5, 0, $6)
         RETURNING id",
    )
    .bind(event_id)
    .bind(subscription_id)
    .bind(channel_id)
    .bind(target)
    .bind(DELIVERY_STATUS_QUEUED)
    .bind(payload)
    .fetch_one(db)
    .await?;

    Ok(delivery_id)
}

async fn finalize_delivery_record(
    db: &sqlx::PgPool,
    delivery_id: i64,
    status: &str,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<&str>,
) -> AppResult<()> {
    let delivered_at = if status == DELIVERY_STATUS_DELIVERED {
        Some(Utc::now())
    } else {
        None
    };

    sqlx::query(
        "UPDATE discovery_notification_deliveries
         SET status = $2,
             attempts = $3,
             response_code = $4,
             last_error = $5,
             delivered_at = $6,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(delivery_id)
    .bind(status)
    .bind(attempts)
    .bind(response_code)
    .bind(last_error)
    .bind(delivered_at)
    .execute(db)
    .await?;

    Ok(())
}

fn render_template(template: &str, payload: &Map<String, Value>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in payload {
        let replacement = match value {
            Value::String(v) => v.clone(),
            Value::Number(v) => v.to_string(),
            Value::Bool(v) => v.to_string(),
            _ => continue,
        };
        let token = format!("{{{{{key}}}}}");
        rendered = rendered.replace(&token, &replacement);
    }
    rendered
}

fn build_fingerprint(item: &DiscoveryInputItem) -> String {
    if item.resource_kind == "container" {
        let cluster = item
            .metadata
            .get("cluster")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .trim()
            .to_ascii_lowercase();
        let namespace = item
            .metadata
            .get("namespace")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .trim()
            .to_ascii_lowercase();
        let pod = item
            .metadata
            .get("pod")
            .and_then(Value::as_str)
            .unwrap_or("pod")
            .trim()
            .to_ascii_lowercase();
        let container = item
            .metadata
            .get("container")
            .and_then(Value::as_str)
            .unwrap_or(item.name.as_str())
            .trim()
            .to_ascii_lowercase();
        return format!("container:{cluster}:{namespace}:{pod}:{container}");
    }

    let hostname = item
        .hostname
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let ip = item.ip.as_deref().unwrap_or("").trim();

    if !hostname.is_empty() && !ip.is_empty() {
        return format!("host:{hostname}:{ip}");
    }
    if !hostname.is_empty() {
        return format!("host:{hostname}");
    }
    if !ip.is_empty() {
        return format!("host:{ip}");
    }

    format!("name:{}", item.name.trim().to_ascii_lowercase())
}

fn build_asset_profile(item: &DiscoveryInputItem) -> Value {
    let mut profile = Map::new();
    profile.insert("name".to_string(), Value::String(item.name.clone()));
    profile.insert(
        "asset_class".to_string(),
        Value::String(item.asset_class.clone()),
    );
    profile.insert(
        "resource_kind".to_string(),
        Value::String(item.resource_kind.clone()),
    );
    if let Some(hostname) = &item.hostname {
        profile.insert("hostname".to_string(), Value::String(hostname.clone()));
    }
    if let Some(ip) = &item.ip {
        profile.insert("ip".to_string(), Value::String(ip.clone()));
    }
    if let Some(metadata_obj) = item.metadata.as_object() {
        let mut metadata = metadata_obj.clone();
        metadata.remove("timestamp");
        metadata.remove("last_seen");
        profile.insert("metadata".to_string(), Value::Object(metadata));
    }
    Value::Object(profile)
}

fn build_candidate_payload(source_type: &str, item: &DiscoveryInputItem) -> Value {
    let mut payload = Map::new();
    payload.insert("name".to_string(), Value::String(item.name.clone()));
    payload.insert(
        "asset_class".to_string(),
        Value::String(item.asset_class.clone()),
    );
    payload.insert(
        "resource_kind".to_string(),
        Value::String(item.resource_kind.clone()),
    );
    payload.insert(
        "source_type".to_string(),
        Value::String(source_type.to_string()),
    );
    if let Some(hostname) = &item.hostname {
        payload.insert("hostname".to_string(), Value::String(hostname.clone()));
    }
    if let Some(ip) = &item.ip {
        payload.insert("ip".to_string(), Value::String(ip.clone()));
    }
    payload.insert("metadata".to_string(), item.metadata.clone());

    Value::Object(payload)
}

async fn pending_candidate_exists(db: &sqlx::PgPool, fingerprint: &str) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM discovery_candidates
            WHERE fingerprint = $1
              AND review_status = 'pending'
        )",
    )
    .bind(fingerprint)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

async fn get_candidate_by_id(
    db: &sqlx::PgPool,
    candidate_id: i64,
) -> AppResult<DiscoveryCandidate> {
    let item: Option<DiscoveryCandidate> = sqlx::query_as(
        "SELECT id, job_id, fingerprint, payload, review_status, discovered_at, reviewed_by, reviewed_at, created_at, updated_at
         FROM discovery_candidates
         WHERE id = $1",
    )
    .bind(candidate_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("discovery candidate {candidate_id} not found")))
}

fn ensure_candidate_pending(candidate: &DiscoveryCandidate) -> AppResult<()> {
    if candidate.review_status == "pending" {
        return Ok(());
    }

    Err(AppError::Validation(format!(
        "candidate {} is already reviewed with status '{}'",
        candidate.id, candidate.review_status
    )))
}

fn normalize_reviewer(value: Option<String>) -> String {
    value
        .and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| "system".to_string())
}

fn append_candidate_filters(builder: &mut QueryBuilder<Postgres>, review_status: Option<String>) {
    if let Some(review_status) = review_status {
        builder
            .push(" AND review_status = ")
            .push_bind(review_status.to_ascii_lowercase());
    }
}

fn append_event_filters(
    builder: &mut QueryBuilder<Postgres>,
    job_id: Option<i64>,
    asset_id: Option<i64>,
    event_type: Option<String>,
    time_from: Option<DateTime<Utc>>,
    time_to: Option<DateTime<Utc>>,
) {
    if let Some(job_id) = job_id {
        builder.push(" AND job_id = ").push_bind(job_id);
    }
    if let Some(asset_id) = asset_id {
        builder.push(" AND asset_id = ").push_bind(asset_id);
    }
    if let Some(event_type) = event_type {
        builder.push(" AND event_type = ").push_bind(event_type);
    }
    if let Some(time_from) = time_from {
        builder.push(" AND happened_at >= ").push_bind(time_from);
    }
    if let Some(time_to) = time_to {
        builder.push(" AND happened_at <= ").push_bind(time_to);
    }
}

fn append_delivery_filters(
    builder: &mut QueryBuilder<Postgres>,
    event_id: Option<i64>,
    status: Option<String>,
) {
    if let Some(event_id) = event_id {
        builder.push(" AND event_id = ").push_bind(event_id);
    }
    if let Some(status) = status {
        builder.push(" AND status = ").push_bind(status);
    }
}

fn parse_time_filter(value: Option<String>, field: &str) -> AppResult<Option<DateTime<Utc>>> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let parsed = DateTime::parse_from_rfc3339(raw.trim()).map_err(|_| {
        AppError::Validation(format!(
            "{field} must be RFC3339 datetime, e.g. 2026-03-02T08:00:00Z"
        ))
    })?;

    Ok(Some(parsed.with_timezone(&Utc)))
}

fn parse_offboarded_threshold(scope: &Value) -> i32 {
    scope
        .get("offboarded_threshold")
        .and_then(Value::as_i64)
        .map(|v| v.clamp(1, 100) as i32)
        .unwrap_or(3)
}

fn required_trimmed(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_source_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed("source_type", value)?.to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "zabbix" | SOURCE_ZABBIX_HOSTS => SOURCE_ZABBIX_HOSTS,
        "snmp" | "snmp_scan" | SOURCE_SNMP_SEED => SOURCE_SNMP_SEED,
        "k8s" | "kubernetes" | SOURCE_K8S_SEED => SOURCE_K8S_SEED,
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported source_type '{}', supported: {}",
                normalized,
                supported_source_types().join(", ")
            )));
        }
    };

    Ok(canonical.to_string())
}

fn normalize_scope(scope: Option<Value>) -> AppResult<Value> {
    let scope = scope.unwrap_or_else(|| Value::Object(Map::new()));
    if !scope.is_object() {
        return Err(AppError::Validation(
            "scope must be a JSON object".to_string(),
        ));
    }
    Ok(scope)
}

fn supported_source_types() -> Vec<&'static str> {
    vec![SOURCE_ZABBIX_HOSTS, SOURCE_SNMP_SEED, SOURCE_K8S_SEED]
}

fn pick_string<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> AppResult<&'a str> {
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    Err(AppError::Validation(format!(
        "missing required string field in keys: {}",
        keys.join(", ")
    )))
}

fn optional_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = object.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn truncate_error(message: &str) -> String {
    const MAX: usize = 1024;
    if message.len() <= MAX {
        message.to_string()
    } else {
        format!("{}...", &message[..MAX])
    }
}
