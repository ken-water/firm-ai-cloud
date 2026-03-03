use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, QueryBuilder};

use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/overview", get(get_monitoring_overview))
        .route("/layers/{layer}", get(get_monitoring_layer))
        .route(
            "/sources",
            get(list_monitoring_sources).post(create_monitoring_source),
        )
        .route(
            "/sources/{id}/probe",
            axum::routing::post(probe_monitoring_source),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MonitoringSource {
    id: i64,
    name: String,
    source_type: String,
    endpoint: String,
    proxy_endpoint: Option<String>,
    auth_type: String,
    username: Option<String>,
    secret_ref: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: bool,
    last_probe_at: Option<DateTime<Utc>>,
    last_probe_status: Option<String>,
    last_probe_message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct MonitoringSourceProbeResponse {
    source: MonitoringSource,
    reachable: bool,
    status_code: Option<u16>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CreateMonitoringSourceRequest {
    name: String,
    source_type: String,
    endpoint: String,
    proxy_endpoint: Option<String>,
    auth_type: Option<String>,
    username: Option<String>,
    secret_ref: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListMonitoringSourcesQuery {
    source_type: Option<String>,
    site: Option<String>,
    department: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct MonitoringOverviewQuery {
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MonitoringLayerQuery {
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct MonitoringScope {
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize)]
struct MonitoringOverviewResponse {
    generated_at: DateTime<Utc>,
    scope: MonitoringScope,
    summary: MonitoringOverviewSummary,
    layers: Vec<MonitoringLayerOverview>,
    empty: bool,
}

#[derive(Debug, Serialize, Default)]
struct MonitoringOverviewSummary {
    source_total: i64,
    source_enabled_total: i64,
    source_reachable_total: i64,
    source_unreachable_total: i64,
    source_unknown_probe_total: i64,
    asset_total: i64,
    monitored_asset_total: i64,
}

#[derive(Debug, Serialize)]
struct MonitoringLayerOverview {
    layer: String,
    asset_total: i64,
    monitored_asset_total: i64,
    health: MonitoringHealthSummary,
    latest_job_statuses: MonitoringJobStatusSummary,
}

#[derive(Debug, Serialize, Default, Clone)]
struct MonitoringHealthSummary {
    healthy: i64,
    warning: i64,
    critical: i64,
    unknown: i64,
}

#[derive(Debug, Serialize, Default, Clone)]
struct MonitoringJobStatusSummary {
    pending: i64,
    running: i64,
    success: i64,
    failed: i64,
    dead_letter: i64,
    skipped: i64,
    unknown: i64,
}

#[derive(Debug, Serialize)]
struct MonitoringLayerResponse {
    generated_at: DateTime<Utc>,
    scope: MonitoringScope,
    layer: String,
    summary: MonitoringLayerSummary,
    items: Vec<MonitoringLayerItem>,
    total: i64,
    limit: u32,
    offset: u32,
    empty: bool,
}

#[derive(Debug, Serialize)]
struct MonitoringLayerSummary {
    asset_total: i64,
    monitored_asset_total: i64,
    health: MonitoringHealthSummary,
    latest_job_statuses: MonitoringJobStatusSummary,
}

#[derive(Debug, Serialize)]
struct MonitoringLayerItem {
    asset_id: i64,
    asset_class: String,
    name: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    monitoring_status: String,
    monitoring_health: String,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_message: Option<String>,
    latest_job_status: Option<String>,
    latest_job_attempt: Option<i32>,
    latest_job_max_attempts: Option<i32>,
    latest_job_requested_at: Option<DateTime<Utc>>,
    latest_job_last_error: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct MonitoringSourceSummaryRow {
    source_total: i64,
    source_enabled_total: i64,
    source_reachable_total: i64,
    source_unreachable_total: i64,
    source_unknown_probe_total: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct MonitoringOverviewAssetRow {
    asset_class: String,
    monitoring_status: Option<String>,
    latest_job_status: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct MonitoringLayerItemRow {
    asset_id: i64,
    asset_class: String,
    name: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    monitoring_status: Option<String>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_message: Option<String>,
    latest_job_status: Option<String>,
    latest_job_attempt: Option<i32>,
    latest_job_max_attempts: Option<i32>,
    latest_job_requested_at: Option<DateTime<Utc>>,
    latest_job_last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MonitoringLayer {
    Hardware,
    Network,
    Service,
    Business,
}

#[derive(Debug)]
struct ProbeResult {
    reachable: bool,
    status_code: Option<u16>,
    message: String,
}

#[derive(Debug, Default, Clone)]
struct MonitoringLayerStats {
    asset_total: i64,
    monitored_asset_total: i64,
    health: MonitoringHealthSummary,
    latest_job_statuses: MonitoringJobStatusSummary,
}

const MONITORING_LAYER_CASE_SQL: &str = "CASE
    WHEN LOWER(a.asset_class) IN ('server', 'physical_host', 'virtual_machine', 'vm', 'baremetal') THEN 'hardware'
    WHEN LOWER(a.asset_class) IN ('network_device', 'switch', 'router', 'firewall', 'load_balancer') THEN 'network'
    WHEN LOWER(a.asset_class) IN ('business_service', 'team', 'business_process', 'application_service') THEN 'business'
    ELSE 'service'
END";

async fn get_monitoring_overview(
    State(state): State<AppState>,
    Query(query): Query<MonitoringOverviewQuery>,
) -> AppResult<Json<MonitoringOverviewResponse>> {
    let site = normalize_scope_filter(query.site, "site", 128)?;
    let department = normalize_scope_filter(query.department, "department", 128)?;

    let source_summary =
        load_monitoring_source_summary(&state.db, site.as_deref(), department.as_deref()).await?;
    let asset_rows =
        load_overview_asset_rows(&state.db, site.as_deref(), department.as_deref()).await?;

    let mut hardware = MonitoringLayerStats::default();
    let mut network = MonitoringLayerStats::default();
    let mut service = MonitoringLayerStats::default();
    let mut business = MonitoringLayerStats::default();

    for row in &asset_rows {
        let layer_stats = match classify_monitoring_layer(&row.asset_class) {
            MonitoringLayer::Hardware => &mut hardware,
            MonitoringLayer::Network => &mut network,
            MonitoringLayer::Service => &mut service,
            MonitoringLayer::Business => &mut business,
        };

        layer_stats.asset_total += 1;
        if row.monitoring_status.is_some() {
            layer_stats.monitored_asset_total += 1;
        }
        increment_health_summary(&mut layer_stats.health, row.monitoring_status.as_deref());
        increment_job_summary(
            &mut layer_stats.latest_job_statuses,
            row.latest_job_status.as_deref(),
        );
    }

    let layers = vec![
        to_layer_overview(MonitoringLayer::Hardware, hardware.clone()),
        to_layer_overview(MonitoringLayer::Network, network.clone()),
        to_layer_overview(MonitoringLayer::Service, service.clone()),
        to_layer_overview(MonitoringLayer::Business, business.clone()),
    ];

    let summary = MonitoringOverviewSummary {
        source_total: source_summary.source_total,
        source_enabled_total: source_summary.source_enabled_total,
        source_reachable_total: source_summary.source_reachable_total,
        source_unreachable_total: source_summary.source_unreachable_total,
        source_unknown_probe_total: source_summary.source_unknown_probe_total,
        asset_total: layers.iter().map(|item| item.asset_total).sum(),
        monitored_asset_total: layers.iter().map(|item| item.monitored_asset_total).sum(),
    };

    Ok(Json(MonitoringOverviewResponse {
        generated_at: Utc::now(),
        scope: MonitoringScope { site, department },
        summary: MonitoringOverviewSummary {
            source_total: summary.source_total,
            source_enabled_total: summary.source_enabled_total,
            source_reachable_total: summary.source_reachable_total,
            source_unreachable_total: summary.source_unreachable_total,
            source_unknown_probe_total: summary.source_unknown_probe_total,
            asset_total: summary.asset_total,
            monitored_asset_total: summary.monitored_asset_total,
        },
        layers,
        empty: summary.asset_total == 0,
    }))
}

async fn get_monitoring_layer(
    State(state): State<AppState>,
    Path(layer): Path<String>,
    Query(query): Query<MonitoringLayerQuery>,
) -> AppResult<Json<MonitoringLayerResponse>> {
    let layer = normalize_monitoring_layer(layer)?;
    let site = normalize_scope_filter(query.site, "site", 128)?;
    let department = normalize_scope_filter(query.department, "department", 128)?;
    let limit = query.limit.unwrap_or(50).min(200) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let total =
        count_layer_assets(&state.db, layer, site.as_deref(), department.as_deref()).await?;
    let summary_stats =
        summarize_layer_assets(&state.db, layer, site.as_deref(), department.as_deref()).await?;
    let rows = fetch_layer_items(
        &state.db,
        layer,
        site.as_deref(),
        department.as_deref(),
        limit,
        offset,
    )
    .await?;

    let items = rows
        .into_iter()
        .map(|row| MonitoringLayerItem {
            asset_id: row.asset_id,
            asset_class: row.asset_class,
            name: row.name,
            status: row.status,
            site: row.site,
            department: row.department,
            monitoring_status: row
                .monitoring_status
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            monitoring_health: map_monitoring_health(row.monitoring_status.as_deref()).to_string(),
            last_sync_at: row.last_sync_at,
            last_sync_message: row.last_sync_message,
            latest_job_status: row.latest_job_status,
            latest_job_attempt: row.latest_job_attempt,
            latest_job_max_attempts: row.latest_job_max_attempts,
            latest_job_requested_at: row.latest_job_requested_at,
            latest_job_last_error: row.latest_job_last_error,
        })
        .collect::<Vec<_>>();

    Ok(Json(MonitoringLayerResponse {
        generated_at: Utc::now(),
        scope: MonitoringScope { site, department },
        layer: layer.as_str().to_string(),
        summary: MonitoringLayerSummary {
            asset_total: summary_stats.asset_total,
            monitored_asset_total: summary_stats.monitored_asset_total,
            health: summary_stats.health,
            latest_job_statuses: summary_stats.latest_job_statuses,
        },
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
        empty: total == 0,
    }))
}

fn to_layer_overview(
    layer: MonitoringLayer,
    stats: MonitoringLayerStats,
) -> MonitoringLayerOverview {
    MonitoringLayerOverview {
        layer: layer.as_str().to_string(),
        asset_total: stats.asset_total,
        monitored_asset_total: stats.monitored_asset_total,
        health: stats.health,
        latest_job_statuses: stats.latest_job_statuses,
    }
}

async fn load_monitoring_source_summary(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<MonitoringSourceSummaryRow> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            COUNT(*) AS source_total,
            COALESCE(SUM(CASE WHEN is_enabled THEN 1 ELSE 0 END), 0) AS source_enabled_total,
            COALESCE(SUM(CASE WHEN last_probe_status = 'reachable' THEN 1 ELSE 0 END), 0) AS source_reachable_total,
            COALESCE(SUM(CASE WHEN last_probe_status = 'unreachable' THEN 1 ELSE 0 END), 0) AS source_unreachable_total,
            COALESCE(SUM(CASE WHEN last_probe_status IS NULL THEN 1 ELSE 0 END), 0) AS source_unknown_probe_total
         FROM monitoring_sources
         WHERE 1=1",
    );

    append_source_scope_filters(&mut qb, site, department);

    qb.build_query_as()
        .fetch_one(db)
        .await
        .map_err(AppError::from)
}

async fn load_overview_asset_rows(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<Vec<MonitoringOverviewAssetRow>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            a.asset_class,
            b.last_sync_status AS monitoring_status,
            j.status AS latest_job_status
         FROM assets a
         LEFT JOIN cmdb_monitoring_bindings b ON b.asset_id = a.id
         LEFT JOIN LATERAL (
            SELECT status
            FROM cmdb_monitoring_sync_jobs
            WHERE asset_id = a.id
            ORDER BY requested_at DESC, id DESC
            LIMIT 1
         ) j ON TRUE
         WHERE 1=1",
    );

    append_asset_scope_filters(&mut qb, site, department);
    qb.push(" ORDER BY a.id DESC");

    qb.build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)
}

async fn count_layer_assets(
    db: &sqlx::PgPool,
    layer: MonitoringLayer,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<i64> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(*)
         FROM assets a
         WHERE 1=1",
    );
    append_asset_scope_filters(&mut qb, site, department);
    append_layer_filter(&mut qb, layer);

    qb.build_query_scalar()
        .fetch_one(db)
        .await
        .map_err(AppError::from)
}

async fn summarize_layer_assets(
    db: &sqlx::PgPool,
    layer: MonitoringLayer,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<MonitoringLayerStats> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            b.last_sync_status AS monitoring_status,
            j.status AS latest_job_status
         FROM assets a
         LEFT JOIN cmdb_monitoring_bindings b ON b.asset_id = a.id
         LEFT JOIN LATERAL (
            SELECT status
            FROM cmdb_monitoring_sync_jobs
            WHERE asset_id = a.id
            ORDER BY requested_at DESC, id DESC
            LIMIT 1
         ) j ON TRUE
         WHERE 1=1",
    );
    append_asset_scope_filters(&mut qb, site, department);
    append_layer_filter(&mut qb, layer);

    let rows: Vec<MonitoringOverviewAssetRow> = qb
        .build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)?;

    let mut stats = MonitoringLayerStats::default();
    for row in rows {
        stats.asset_total += 1;
        if row.monitoring_status.is_some() {
            stats.monitored_asset_total += 1;
        }
        increment_health_summary(&mut stats.health, row.monitoring_status.as_deref());
        increment_job_summary(
            &mut stats.latest_job_statuses,
            row.latest_job_status.as_deref(),
        );
    }

    Ok(stats)
}

async fn fetch_layer_items(
    db: &sqlx::PgPool,
    layer: MonitoringLayer,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<MonitoringLayerItemRow>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            a.id AS asset_id,
            a.asset_class,
            a.name,
            a.status,
            a.site,
            a.department,
            b.last_sync_status AS monitoring_status,
            b.last_sync_at,
            b.last_sync_message,
            j.status AS latest_job_status,
            j.attempt AS latest_job_attempt,
            j.max_attempts AS latest_job_max_attempts,
            j.requested_at AS latest_job_requested_at,
            j.last_error AS latest_job_last_error
         FROM assets a
         LEFT JOIN cmdb_monitoring_bindings b ON b.asset_id = a.id
         LEFT JOIN LATERAL (
            SELECT status, attempt, max_attempts, requested_at, last_error
            FROM cmdb_monitoring_sync_jobs
            WHERE asset_id = a.id
            ORDER BY requested_at DESC, id DESC
            LIMIT 1
         ) j ON TRUE
         WHERE 1=1",
    );
    append_asset_scope_filters(&mut qb, site, department);
    append_layer_filter(&mut qb, layer);
    qb.push(" ORDER BY a.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    qb.build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)
}

fn append_source_scope_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    site: Option<&'a str>,
    department: Option<&'a str>,
) {
    if let Some(site) = site {
        builder.push(" AND site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND department = ").push_bind(department);
    }
}

fn append_asset_scope_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    site: Option<&'a str>,
    department: Option<&'a str>,
) {
    if let Some(site) = site {
        builder.push(" AND a.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND a.department = ").push_bind(department);
    }
}

fn append_layer_filter(builder: &mut QueryBuilder<Postgres>, layer: MonitoringLayer) {
    builder
        .push(" AND ")
        .push(MONITORING_LAYER_CASE_SQL)
        .push(" = ");
    builder.push_bind(layer.as_str());
}

fn normalize_monitoring_layer(value: String) -> AppResult<MonitoringLayer> {
    let normalized = required_trimmed("layer", value, 32)?.to_ascii_lowercase();
    match normalized.as_str() {
        "hardware" => Ok(MonitoringLayer::Hardware),
        "network" => Ok(MonitoringLayer::Network),
        "service" => Ok(MonitoringLayer::Service),
        "business" => Ok(MonitoringLayer::Business),
        _ => Err(AppError::Validation(
            "layer must be one of: hardware, network, service, business".to_string(),
        )),
    }
}

fn classify_monitoring_layer(asset_class: &str) -> MonitoringLayer {
    let normalized = asset_class.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "server" | "physical_host" | "virtual_machine" | "vm" | "baremetal" => {
            MonitoringLayer::Hardware
        }
        "network_device" | "switch" | "router" | "firewall" | "load_balancer" => {
            MonitoringLayer::Network
        }
        "business_service" | "team" | "business_process" | "application_service" => {
            MonitoringLayer::Business
        }
        _ => MonitoringLayer::Service,
    }
}

fn normalize_scope_filter(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let normalized = trim_optional(value);
    if let Some(ref value) = normalized {
        if value.len() > max_len {
            return Err(AppError::Validation(format!(
                "{field} length must be <= {max_len}"
            )));
        }
    }
    Ok(normalized)
}

fn map_monitoring_health(status: Option<&str>) -> &'static str {
    let Some(status) = status else {
        return "unknown";
    };

    match status.trim().to_ascii_lowercase().as_str() {
        "success" => "healthy",
        "pending" | "running" => "warning",
        "failed" | "dead_letter" => "critical",
        "skipped" => "unknown",
        _ => "unknown",
    }
}

fn increment_health_summary(summary: &mut MonitoringHealthSummary, status: Option<&str>) {
    match map_monitoring_health(status) {
        "healthy" => summary.healthy += 1,
        "warning" => summary.warning += 1,
        "critical" => summary.critical += 1,
        _ => summary.unknown += 1,
    }
}

fn increment_job_summary(summary: &mut MonitoringJobStatusSummary, status: Option<&str>) {
    let Some(status) = status else {
        summary.unknown += 1;
        return;
    };

    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => summary.pending += 1,
        "running" => summary.running += 1,
        "success" => summary.success += 1,
        "failed" => summary.failed += 1,
        "dead_letter" => summary.dead_letter += 1,
        "skipped" => summary.skipped += 1,
        _ => summary.unknown += 1,
    }
}

async fn list_monitoring_sources(
    State(state): State<AppState>,
    Query(query): Query<ListMonitoringSourcesQuery>,
) -> AppResult<Json<Vec<MonitoringSource>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at
         FROM monitoring_sources
         WHERE 1=1",
    );

    if let Some(source_type) = trim_optional(query.source_type) {
        builder.push(" AND source_type = ").push_bind(source_type);
    }
    if let Some(site) = trim_optional(query.site) {
        builder.push(" AND site = ").push_bind(site);
    }
    if let Some(department) = trim_optional(query.department) {
        builder.push(" AND department = ").push_bind(department);
    }
    if let Some(is_enabled) = query.is_enabled {
        builder.push(" AND is_enabled = ").push_bind(is_enabled);
    }

    builder.push(" ORDER BY id DESC");
    let items: Vec<MonitoringSource> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn create_monitoring_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateMonitoringSourceRequest>,
) -> AppResult<Json<MonitoringSource>> {
    let name = required_trimmed("name", payload.name, 128)?;
    let source_type = normalize_source_type(payload.source_type)?;
    let endpoint = normalize_endpoint(payload.endpoint)?;
    let proxy_endpoint = normalize_optional_endpoint(payload.proxy_endpoint)?;
    let auth_type = normalize_auth_type(payload.auth_type)?;
    let username = normalize_username(payload.username, &auth_type)?;
    let secret_ref = required_trimmed("secret_ref", payload.secret_ref, 255)?;
    let site = trim_optional(payload.site);
    let department = trim_optional(payload.department);
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let item: MonitoringSource = sqlx::query_as(
        "INSERT INTO monitoring_sources (
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at",
    )
    .bind(name)
    .bind(source_type)
    .bind(endpoint)
    .bind(proxy_endpoint)
    .bind(auth_type)
    .bind(username)
    .bind(secret_ref)
    .bind(site)
    .bind(department)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_create_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "monitoring.source.create",
        "monitoring_source",
        Some(item.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "source_type": &item.source_type,
            "endpoint": &item.endpoint,
            "is_enabled": item.is_enabled
        }),
    )
    .await;

    Ok(Json(item))
}

async fn probe_monitoring_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<MonitoringSourceProbeResponse>> {
    let source = get_monitoring_source(&state.db, id).await?;
    let probe = probe_endpoint(&source.endpoint).await;
    let status = if probe.reachable {
        "reachable"
    } else {
        "unreachable"
    };

    let updated: MonitoringSource = sqlx::query_as(
        "UPDATE monitoring_sources
         SET
            last_probe_at = NOW(),
            last_probe_status = $2,
            last_probe_message = $3,
            updated_at = NOW()
         WHERE id = $1
         RETURNING
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at",
    )
    .bind(id)
    .bind(status)
    .bind(limit_len(&probe.message, 512))
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "monitoring.source.probe",
        "monitoring_source",
        Some(id.to_string()),
        if probe.reachable { "success" } else { "failed" },
        Some(probe.message.clone()),
        serde_json::json!({
            "reachable": probe.reachable,
            "status_code": probe.status_code
        }),
    )
    .await;

    Ok(Json(MonitoringSourceProbeResponse {
        source: updated,
        reachable: probe.reachable,
        status_code: probe.status_code,
        message: probe.message,
    }))
}

async fn get_monitoring_source(db: &sqlx::PgPool, source_id: i64) -> AppResult<MonitoringSource> {
    let item: Option<MonitoringSource> = sqlx::query_as(
        "SELECT
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at
         FROM monitoring_sources
         WHERE id = $1",
    )
    .bind(source_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("monitoring source {source_id} does not exist")))
}

async fn probe_endpoint(endpoint: &str) -> ProbeResult {
    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(response) => ProbeResult {
            reachable: true,
            status_code: Some(response.status().as_u16()),
            message: format!("probe request succeeded with HTTP {}", response.status()),
        },
        Err(err) => ProbeResult {
            reachable: false,
            status_code: None,
            message: format!("probe request failed: {err}"),
        },
    }
}

impl MonitoringLayer {
    fn as_str(self) -> &'static str {
        match self {
            MonitoringLayer::Hardware => "hardware",
            MonitoringLayer::Network => "network",
            MonitoringLayer::Service => "service",
            MonitoringLayer::Business => "business",
        }
    }
}

fn normalize_source_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed("source_type", value, 32)?.to_ascii_lowercase();
    if normalized != "zabbix" {
        return Err(AppError::Validation(
            "source_type must be 'zabbix'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_auth_type(value: Option<String>) -> AppResult<String> {
    let normalized = value
        .and_then(|raw| trim_optional(Some(raw)))
        .unwrap_or_else(|| "token".to_string())
        .to_ascii_lowercase();

    match normalized.as_str() {
        "token" | "basic" => Ok(normalized),
        _ => Err(AppError::Validation(
            "auth_type must be one of: token, basic".to_string(),
        )),
    }
}

fn normalize_username(value: Option<String>, auth_type: &str) -> AppResult<Option<String>> {
    let username = trim_optional(value);
    if auth_type == "basic" && username.is_none() {
        return Err(AppError::Validation(
            "username is required when auth_type=basic".to_string(),
        ));
    }

    if let Some(ref user) = username {
        if user.len() > 128 {
            return Err(AppError::Validation(
                "username length must be <= 128".to_string(),
            ));
        }
    }
    Ok(username)
}

fn normalize_endpoint(value: String) -> AppResult<String> {
    let endpoint = required_trimmed("endpoint", value, 512)?;
    validate_url(&endpoint, "endpoint")?;
    Ok(endpoint)
}

fn normalize_optional_endpoint(value: Option<String>) -> AppResult<Option<String>> {
    let endpoint = trim_optional(value);
    if let Some(ref endpoint) = endpoint {
        if endpoint.len() > 512 {
            return Err(AppError::Validation(
                "proxy_endpoint length must be <= 512".to_string(),
            ));
        }
        validate_url(endpoint, "proxy_endpoint")?;
    }
    Ok(endpoint)
}

fn validate_url(value: &str, field: &str) -> AppResult<()> {
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation(format!("{field} must be a valid URL")))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::Validation(format!(
            "{field} must use http or https scheme"
        )));
    }

    Ok(())
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

fn limit_len(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    value.chars().take(max_len).collect()
}

fn map_create_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("monitoring source name already exists".to_string());
        }
    }
    AppError::Database(err)
}

#[cfg(test)]
mod tests {
    use super::{
        map_monitoring_health, normalize_auth_type, normalize_monitoring_layer,
        normalize_optional_endpoint, normalize_scope_filter, normalize_source_type,
        normalize_username,
    };

    #[test]
    fn validates_source_type() {
        assert!(normalize_source_type("zabbix".to_string()).is_ok());
        assert!(normalize_source_type("snmp".to_string()).is_err());
    }

    #[test]
    fn validates_auth_type() {
        assert!(normalize_auth_type(None).is_ok());
        assert!(normalize_auth_type(Some("basic".to_string())).is_ok());
        assert!(normalize_auth_type(Some("oauth".to_string())).is_err());
    }

    #[test]
    fn basic_auth_requires_username() {
        assert!(normalize_username(None, "basic").is_err());
        assert!(normalize_username(Some("ops".to_string()), "basic").is_ok());
    }

    #[test]
    fn validates_optional_proxy_url() {
        assert!(normalize_optional_endpoint(Some("http://127.0.0.1:8080".to_string())).is_ok());
        assert!(normalize_optional_endpoint(Some("ftp://host".to_string())).is_err());
    }

    #[test]
    fn validates_monitoring_layer_filter() {
        assert!(normalize_monitoring_layer("hardware".to_string()).is_ok());
        assert!(normalize_monitoring_layer("network".to_string()).is_ok());
        assert!(normalize_monitoring_layer("service".to_string()).is_ok());
        assert!(normalize_monitoring_layer("business".to_string()).is_ok());
        assert!(normalize_monitoring_layer("unsupported".to_string()).is_err());
    }

    #[test]
    fn normalizes_scope_filter() {
        assert_eq!(
            normalize_scope_filter(Some("   dc-a  ".to_string()), "site", 128)
                .expect("scope normalization"),
            Some("dc-a".to_string())
        );
        assert!(normalize_scope_filter(Some(" ".to_string()), "site", 128).is_ok());
    }

    #[test]
    fn maps_monitoring_health_states() {
        assert_eq!(map_monitoring_health(Some("success")), "healthy");
        assert_eq!(map_monitoring_health(Some("running")), "warning");
        assert_eq!(map_monitoring_health(Some("failed")), "critical");
        assert_eq!(map_monitoring_health(Some("skipped")), "unknown");
        assert_eq!(map_monitoring_health(None), "unknown");
    }
}
