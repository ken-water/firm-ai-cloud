use std::time::Duration;

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

use crate::error::{AppError, AppResult};
use crate::state::AppState;

const SOURCE_ZABBIX_HOSTS: &str = "zabbix_hosts";
const SOURCE_SNMP_SEED: &str = "snmp_seed";
const SOURCE_K8S_SEED: &str = "k8s_seed";

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

#[derive(Debug, Serialize)]
struct ListDiscoveryCandidatesResponse {
    items: Vec<DiscoveryCandidate>,
    total: i64,
    limit: u32,
    offset: u32,
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

async fn execute_discovery_job(
    db: &sqlx::PgPool,
    job: &DiscoveryJob,
) -> AppResult<DiscoveryRunStats> {
    let items = collect_discovery_items(job).await?;
    let mut stats = DiscoveryRunStats::default();

    for item in items {
        let fingerprint = build_fingerprint(&item);

        if let (Some(hostname), Some(ip)) = (&item.hostname, &item.ip) {
            if asset_exists_by_hostname_ip(db, hostname, ip).await? {
                stats.matched_assets += 1;
                continue;
            }
        }

        if pending_candidate_exists(db, &fingerprint).await? {
            stats.skipped_candidates += 1;
            continue;
        }

        let payload = build_candidate_payload(job.source_type.as_str(), &item);

        sqlx::query(
            "INSERT INTO discovery_candidates (job_id, fingerprint, payload, review_status)
             VALUES ($1, $2, $3, 'pending')",
        )
        .bind(job.id)
        .bind(fingerprint)
        .bind(payload)
        .execute(db)
        .await?;

        stats.queued_candidates += 1;
    }

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

async fn asset_exists_by_hostname_ip(
    db: &sqlx::PgPool,
    hostname: &str,
    ip: &str,
) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM assets
            WHERE LOWER(COALESCE(hostname, '')) = LOWER($1)
              AND COALESCE(ip, '') = $2
        )",
    )
    .bind(hostname)
    .bind(ip)
    .fetch_one(db)
    .await?;

    Ok(exists)
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

fn append_candidate_filters(builder: &mut QueryBuilder<Postgres>, review_status: Option<String>) {
    if let Some(review_status) = review_status {
        builder
            .push(" AND review_status = ")
            .push_bind(review_status.to_ascii_lowercase());
    }
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
