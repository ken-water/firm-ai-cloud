use std::collections::HashSet;

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
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const AUTH_SITE_HEADER: &str = "x-auth-site";
const AUTH_DEPARTMENT_HEADER: &str = "x-auth-department";
const TOPOLOGY_LIMIT_DEFAULT: u32 = 200;
const TOPOLOGY_OFFSET_DEFAULT: u32 = 0;
const DIAGNOSTICS_WINDOW_MINUTES_DEFAULT: u32 = 120;
const DIAGNOSTICS_WINDOW_MINUTES_MAX: u32 = 1_440;
const TOPOLOGY_RELATION_TYPES: [&str; 4] = ["contains", "depends_on", "runs_service", "owned_by"];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/maps/{scope}", get(get_topology_map))
        .route(
            "/diagnostics/edges/{edge_id}",
            axum::routing::get(get_edge_diagnostics),
        )
}

#[derive(Debug, Deserialize, Default)]
struct TopologyMapQuery {
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct EdgeDiagnosticsQuery {
    window_minutes: Option<u32>,
}

#[derive(Debug, Serialize)]
struct TopologyMapResponse {
    generated_at: DateTime<Utc>,
    scope: TopologyScope,
    window: TopologyWindow,
    stats: TopologyMapStats,
    nodes: Vec<TopologyMapNode>,
    edges: Vec<TopologyMapEdge>,
    empty: bool,
}

#[derive(Debug, Serialize)]
struct TopologyScope {
    scope_key: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize)]
struct TopologyWindow {
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct TopologyMapStats {
    total_nodes: i64,
    window_nodes: usize,
    window_edges: usize,
}

#[derive(Debug, Serialize)]
struct TopologyMapNode {
    id: i64,
    name: String,
    asset_class: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    monitoring_status: Option<String>,
    latest_job_status: Option<String>,
    health: String,
}

#[derive(Debug, Serialize)]
struct TopologyMapEdge {
    id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
}

#[derive(Debug, sqlx::FromRow)]
struct TopologyMapNodeRow {
    id: i64,
    name: String,
    asset_class: String,
    status: String,
    site: Option<String>,
    department: Option<String>,
    monitoring_status: Option<String>,
    latest_job_status: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct TopologyMapEdgeRow {
    id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
}

#[derive(Debug, sqlx::FromRow)]
struct EdgeDiagnosticsContextRow {
    edge_id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
    src_name: String,
    dst_name: String,
    src_site: Option<String>,
    src_department: Option<String>,
    dst_site: Option<String>,
    dst_department: Option<String>,
}

#[derive(Debug, Serialize)]
struct EdgeDiagnosticsResponse {
    generated_at: DateTime<Utc>,
    window_minutes: u32,
    relation: EdgeDiagnosticsRelation,
    trend: Vec<EdgeDiagnosticsTrendPoint>,
    alerts: Vec<EdgeDiagnosticsAlert>,
    recent_changes: Vec<EdgeDiagnosticsChange>,
    impacted: EdgeDiagnosticsImpactedHints,
    checklist: Vec<EdgeDiagnosticsChecklistStep>,
    quick_actions: Vec<EdgeDiagnosticsQuickAction>,
}

#[derive(Debug, Serialize)]
struct EdgeDiagnosticsRelation {
    edge_id: i64,
    src_asset_id: i64,
    src_name: String,
    dst_asset_id: i64,
    dst_name: String,
    relation_type: String,
    source: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EdgeDiagnosticsTrendPoint {
    bucket_at: DateTime<Utc>,
    total_jobs: i64,
    failed_jobs: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EdgeDiagnosticsAlert {
    id: i64,
    alert_source: String,
    alert_key: String,
    title: String,
    severity: String,
    status: String,
    asset_id: Option<i64>,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EdgeDiagnosticsChange {
    id: i64,
    actor: String,
    action: String,
    target_type: String,
    target_id: Option<String>,
    result: String,
    message: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct EdgeDiagnosticsImpactedHints {
    services: Vec<EdgeDiagnosticsAssetHint>,
    owners: Vec<EdgeDiagnosticsAssetHint>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EdgeDiagnosticsAssetHint {
    id: i64,
    name: String,
    asset_class: String,
    site: Option<String>,
    department: Option<String>,
}

#[derive(Debug, Serialize)]
struct EdgeDiagnosticsChecklistStep {
    key: String,
    title: String,
    done: bool,
    hint: String,
}

#[derive(Debug, Serialize)]
struct EdgeDiagnosticsQuickAction {
    key: String,
    label: String,
    href: Option<String>,
    api_path: Option<String>,
    method: Option<String>,
    body: Option<Value>,
    requires_write: bool,
}

async fn get_topology_map(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(scope): Path<String>,
    Query(query): Query<TopologyMapQuery>,
) -> AppResult<Json<TopologyMapResponse>> {
    let user = resolve_auth_user(&state, &headers).await?;
    let roles = load_user_roles(&state.db, &user).await?;

    let (scope_site, scope_department, scope_key) = parse_scope_key(scope.as_str())?;
    let mut site = merge_scope_filter(scope_site, query.site, "site")?;
    let mut department = merge_scope_filter(scope_department, query.department, "department")?;
    enforce_scope_access(&roles, &headers, &mut site, &mut department)?;

    let limit = query.limit.unwrap_or(TOPOLOGY_LIMIT_DEFAULT).clamp(10, 500);
    let offset = query.offset.unwrap_or(TOPOLOGY_OFFSET_DEFAULT);
    let total_nodes =
        count_topology_nodes(&state.db, site.as_deref(), department.as_deref()).await?;

    let nodes = fetch_topology_nodes(
        &state.db,
        site.as_deref(),
        department.as_deref(),
        limit as i64,
        offset as i64,
    )
    .await?;
    let node_ids = nodes.iter().map(|item| item.id).collect::<HashSet<_>>();
    let edges = fetch_topology_edges(&state.db, &node_ids).await?;

    Ok(Json(TopologyMapResponse {
        generated_at: Utc::now(),
        scope: TopologyScope {
            scope_key,
            site,
            department,
        },
        window: TopologyWindow { limit, offset },
        stats: TopologyMapStats {
            total_nodes,
            window_nodes: nodes.len(),
            window_edges: edges.len(),
        },
        empty: nodes.is_empty(),
        nodes,
        edges,
    }))
}

async fn get_edge_diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(edge_id): Path<i64>,
    Query(query): Query<EdgeDiagnosticsQuery>,
) -> AppResult<Json<EdgeDiagnosticsResponse>> {
    if edge_id <= 0 {
        return Err(AppError::Validation(
            "edge_id must be a positive integer".to_string(),
        ));
    }

    let window_minutes = normalize_diagnostics_window_minutes(query.window_minutes)?;
    let user = resolve_auth_user(&state, &headers).await?;
    let roles = load_user_roles(&state.db, &user).await?;

    let context = load_edge_diagnostics_context(&state.db, edge_id).await?;

    let mut site = context
        .src_site
        .clone()
        .or_else(|| context.dst_site.clone());
    let mut department = context
        .src_department
        .clone()
        .or_else(|| context.dst_department.clone());
    enforce_scope_access(&roles, &headers, &mut site, &mut department)?;

    let asset_ids = vec![context.src_asset_id, context.dst_asset_id];
    let trend = load_edge_diagnostics_trend(&state.db, &asset_ids, window_minutes).await?;
    let alerts = load_edge_diagnostics_alerts(&state.db, &asset_ids).await?;
    let recent_changes = load_edge_diagnostics_changes(&state.db, edge_id, &asset_ids).await?;
    let impacted = load_edge_diagnostics_impacted_hints(&state.db, &asset_ids).await?;
    let checklist =
        build_edge_diagnostics_checklist(&context.relation_type, &trend, &alerts, &recent_changes);
    let quick_actions = build_edge_diagnostics_quick_actions(edge_id);

    Ok(Json(EdgeDiagnosticsResponse {
        generated_at: Utc::now(),
        window_minutes,
        relation: EdgeDiagnosticsRelation {
            edge_id: context.edge_id,
            src_asset_id: context.src_asset_id,
            src_name: context.src_name,
            dst_asset_id: context.dst_asset_id,
            dst_name: context.dst_name,
            relation_type: context.relation_type,
            source: context.source,
            site,
            department,
        },
        trend,
        alerts,
        recent_changes,
        impacted,
        checklist,
        quick_actions,
    }))
}

fn normalize_diagnostics_window_minutes(value: Option<u32>) -> AppResult<u32> {
    let value = value.unwrap_or(DIAGNOSTICS_WINDOW_MINUTES_DEFAULT);
    if value < 15 || value > DIAGNOSTICS_WINDOW_MINUTES_MAX {
        return Err(AppError::Validation(format!(
            "window_minutes must be between 15 and {DIAGNOSTICS_WINDOW_MINUTES_MAX}"
        )));
    }
    Ok(value)
}

async fn load_edge_diagnostics_context(
    db: &sqlx::PgPool,
    edge_id: i64,
) -> AppResult<EdgeDiagnosticsContextRow> {
    let row: Option<EdgeDiagnosticsContextRow> = sqlx::query_as(
        "SELECT
            r.id AS edge_id,
            r.src_asset_id,
            r.dst_asset_id,
            r.relation_type,
            r.source,
            src.name AS src_name,
            dst.name AS dst_name,
            src.site AS src_site,
            src.department AS src_department,
            dst.site AS dst_site,
            dst.department AS dst_department
         FROM asset_relations r
         INNER JOIN assets src ON src.id = r.src_asset_id
         INNER JOIN assets dst ON dst.id = r.dst_asset_id
         WHERE r.id = $1",
    )
    .bind(edge_id)
    .fetch_optional(db)
    .await?;

    row.ok_or_else(|| AppError::NotFound(format!("topology edge {edge_id} not found")))
}

async fn load_edge_diagnostics_trend(
    db: &sqlx::PgPool,
    asset_ids: &[i64],
    window_minutes: u32,
) -> AppResult<Vec<EdgeDiagnosticsTrendPoint>> {
    sqlx::query_as(
        "SELECT
            date_trunc('hour', requested_at) AS bucket_at,
            COUNT(*)::BIGINT AS total_jobs,
            COUNT(*) FILTER (WHERE status IN ('failed', 'dead_letter'))::BIGINT AS failed_jobs
         FROM cmdb_monitoring_sync_jobs
         WHERE asset_id = ANY($1)
           AND requested_at >= NOW() - ($2::int * INTERVAL '1 minute')
         GROUP BY date_trunc('hour', requested_at)
         ORDER BY bucket_at DESC
         LIMIT 48",
    )
    .bind(asset_ids)
    .bind(window_minutes as i32)
    .fetch_all(db)
    .await
    .map_err(AppError::from)
}

async fn load_edge_diagnostics_alerts(
    db: &sqlx::PgPool,
    asset_ids: &[i64],
) -> AppResult<Vec<EdgeDiagnosticsAlert>> {
    sqlx::query_as(
        "SELECT
            id,
            alert_source,
            alert_key,
            title,
            severity,
            status,
            asset_id,
            last_seen_at
         FROM unified_alerts
         WHERE asset_id = ANY($1)
           AND status IN ('open', 'acknowledged')
         ORDER BY last_seen_at DESC, id DESC
         LIMIT 20",
    )
    .bind(asset_ids)
    .fetch_all(db)
    .await
    .map_err(AppError::from)
}

async fn load_edge_diagnostics_changes(
    db: &sqlx::PgPool,
    edge_id: i64,
    asset_ids: &[i64],
) -> AppResult<Vec<EdgeDiagnosticsChange>> {
    let asset_ids_text = asset_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    sqlx::query_as(
        "SELECT
            id,
            actor,
            action,
            target_type,
            target_id,
            result,
            message,
            created_at
         FROM audit_logs
         WHERE (target_type = 'asset_relation' AND target_id = $1)
            OR (target_type = 'asset' AND target_id = ANY($2))
         ORDER BY created_at DESC, id DESC
         LIMIT 30",
    )
    .bind(edge_id.to_string())
    .bind(asset_ids_text)
    .fetch_all(db)
    .await
    .map_err(AppError::from)
}

async fn load_edge_diagnostics_impacted_hints(
    db: &sqlx::PgPool,
    asset_ids: &[i64],
) -> AppResult<EdgeDiagnosticsImpactedHints> {
    let services = sqlx::query_as(
        "SELECT DISTINCT a.id, a.name, a.asset_class, a.site, a.department
         FROM asset_relations r
         INNER JOIN assets a
            ON (
                (r.src_asset_id = ANY($1) AND a.id = r.dst_asset_id)
                OR (r.dst_asset_id = ANY($1) AND a.id = r.src_asset_id)
            )
         WHERE r.relation_type = 'runs_service'
         ORDER BY a.name ASC
         LIMIT 20",
    )
    .bind(asset_ids)
    .fetch_all(db)
    .await?;

    let owners = sqlx::query_as(
        "SELECT DISTINCT a.id, a.name, a.asset_class, a.site, a.department
         FROM asset_relations r
         INNER JOIN assets a
            ON (
                (r.src_asset_id = ANY($1) AND a.id = r.dst_asset_id)
                OR (r.dst_asset_id = ANY($1) AND a.id = r.src_asset_id)
            )
         WHERE r.relation_type = 'owned_by'
         ORDER BY a.name ASC
         LIMIT 20",
    )
    .bind(asset_ids)
    .fetch_all(db)
    .await?;

    Ok(EdgeDiagnosticsImpactedHints { services, owners })
}

fn build_edge_diagnostics_checklist(
    relation_type: &str,
    trend: &[EdgeDiagnosticsTrendPoint],
    alerts: &[EdgeDiagnosticsAlert],
    recent_changes: &[EdgeDiagnosticsChange],
) -> Vec<EdgeDiagnosticsChecklistStep> {
    let failed_jobs = trend.iter().map(|item| item.failed_jobs).sum::<i64>();
    let has_critical_alert = alerts.iter().any(|item| item.severity == "critical");
    let has_recent_write = recent_changes.iter().any(|item| item.result == "success");

    vec![
        EdgeDiagnosticsChecklistStep {
            key: "scope-confirmed".to_string(),
            title: "Confirm affected scope".to_string(),
            done: true,
            hint: format!("relation_type={relation_type}"),
        },
        EdgeDiagnosticsChecklistStep {
            key: "alerts-triaged".to_string(),
            title: "Triage active alerts".to_string(),
            done: !has_critical_alert,
            hint: if has_critical_alert {
                "Critical alerts are still open around this edge.".to_string()
            } else {
                "No critical alerts remain open.".to_string()
            },
        },
        EdgeDiagnosticsChecklistStep {
            key: "sync-health-reviewed".to_string(),
            title: "Review sync failure trend".to_string(),
            done: failed_jobs == 0,
            hint: format!("failed_jobs_last_window={failed_jobs}"),
        },
        EdgeDiagnosticsChecklistStep {
            key: "changes-correlated".to_string(),
            title: "Correlate with recent changes".to_string(),
            done: has_recent_write,
            hint: if has_recent_write {
                "Recent successful change activity exists and should be correlated.".to_string()
            } else {
                "No recent write activity found for this relation/assets.".to_string()
            },
        },
    ]
}

fn build_edge_diagnostics_quick_actions(edge_id: i64) -> Vec<EdgeDiagnosticsQuickAction> {
    vec![
        EdgeDiagnosticsQuickAction {
            key: "open-alert-center".to_string(),
            label: "Open Alert Center".to_string(),
            href: Some("#/alerts".to_string()),
            api_path: None,
            method: None,
            body: None,
            requires_write: false,
        },
        EdgeDiagnosticsQuickAction {
            key: "open-ticket-center".to_string(),
            label: "Open Ticket Center".to_string(),
            href: Some("#/tickets".to_string()),
            api_path: None,
            method: None,
            body: None,
            requires_write: false,
        },
        EdgeDiagnosticsQuickAction {
            key: "open-playbook-library".to_string(),
            label: "Open Playbook Library".to_string(),
            href: Some("#/workflow".to_string()),
            api_path: None,
            method: None,
            body: None,
            requires_write: true,
        },
        EdgeDiagnosticsQuickAction {
            key: "run-diagnostics-playbook".to_string(),
            label: "Dry-Run Topology Diagnostics Playbook".to_string(),
            href: None,
            api_path: Some(
                "/api/v1/workflow/playbooks/collect-topology-diagnostics/dry-run".to_string(),
            ),
            method: Some("POST".to_string()),
            body: Some(json!({
                "params": {
                    "edge_id": edge_id,
                    "window_minutes": DIAGNOSTICS_WINDOW_MINUTES_DEFAULT,
                    "include_changes": true
                }
            })),
            requires_write: true,
        },
    ]
}

fn parse_scope_key(scope: &str) -> AppResult<(Option<String>, Option<String>, String)> {
    let normalized = scope.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "scope key cannot be empty".to_string(),
        ));
    }

    if normalized.eq_ignore_ascii_case("global") || normalized.eq_ignore_ascii_case("all") {
        return Ok((None, None, "global".to_string()));
    }

    if let Some(value) = normalized.strip_prefix("site:") {
        let value = normalize_scope_value(Some(value.to_string()), "site", 128)?
            .ok_or_else(|| AppError::Validation("site scope cannot be empty".to_string()))?;
        return Ok((Some(value.clone()), None, format!("site:{value}")));
    }

    if let Some(value) = normalized.strip_prefix("department:") {
        let value = normalize_scope_value(Some(value.to_string()), "department", 128)?
            .ok_or_else(|| AppError::Validation("department scope cannot be empty".to_string()))?;
        return Ok((None, Some(value.clone()), format!("department:{value}")));
    }

    Err(AppError::Validation(
        "scope must be one of: global | site:<value> | department:<value>".to_string(),
    ))
}

fn merge_scope_filter(
    from_scope: Option<String>,
    from_query: Option<String>,
    field: &str,
) -> AppResult<Option<String>> {
    let from_query = normalize_scope_value(from_query, field, 128)?;
    match (from_scope, from_query) {
        (Some(scope_value), Some(query_value)) => {
            if scope_value.eq_ignore_ascii_case(&query_value) {
                Ok(Some(scope_value))
            } else {
                Err(AppError::Validation(format!(
                    "{field} query filter conflicts with scope path"
                )))
            }
        }
        (Some(scope_value), None) => Ok(Some(scope_value)),
        (None, Some(query_value)) => Ok(Some(query_value)),
        (None, None) => Ok(None),
    }
}

fn normalize_scope_value(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let normalized = value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    if let Some(ref value) = normalized {
        if value.len() > max_len {
            return Err(AppError::Validation(format!(
                "{field} length must be <= {max_len}"
            )));
        }
    }
    Ok(normalized)
}

fn read_scope_header(headers: &HeaderMap, key: &str, max_len: usize) -> AppResult<Option<String>> {
    let Some(raw) = headers.get(key) else {
        return Ok(None);
    };

    let value = raw
        .to_str()
        .map_err(|_| AppError::Forbidden(format!("{key} header is invalid")))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Forbidden(format!("{key} header cannot be empty")));
    }
    if value.len() > max_len {
        return Err(AppError::Forbidden(format!(
            "{key} header length must be <= {max_len}"
        )));
    }
    Ok(Some(value.to_string()))
}

fn enforce_scope_access(
    roles: &[String],
    headers: &HeaderMap,
    site: &mut Option<String>,
    department: &mut Option<String>,
) -> AppResult<()> {
    if roles.iter().any(|role| role == "admin") {
        return Ok(());
    }

    if site.is_none() && department.is_none() {
        return Err(AppError::Forbidden(
            "scope denied: non-admin topology requests must include site or department filter"
                .to_string(),
        ));
    }

    if let Some(allowed_site) = read_scope_header(headers, AUTH_SITE_HEADER, 128)? {
        match site.clone() {
            Some(requested) if !requested.eq_ignore_ascii_case(&allowed_site) => {
                return Err(AppError::Forbidden(format!(
                    "scope denied: requested site '{}' is outside authorized site '{}'",
                    requested, allowed_site
                )));
            }
            None => *site = Some(allowed_site),
            _ => {}
        }
    }

    if let Some(allowed_department) = read_scope_header(headers, AUTH_DEPARTMENT_HEADER, 128)? {
        match department.clone() {
            Some(requested) if !requested.eq_ignore_ascii_case(&allowed_department) => {
                return Err(AppError::Forbidden(format!(
                    "scope denied: requested department '{}' is outside authorized department '{}'",
                    requested, allowed_department
                )));
            }
            None => *department = Some(allowed_department),
            _ => {}
        }
    }

    Ok(())
}

async fn load_user_roles(db: &sqlx::PgPool, username: &str) -> AppResult<Vec<String>> {
    sqlx::query_scalar(
        "SELECT r.role_key
         FROM iam_users u
         INNER JOIN iam_user_roles ur ON ur.user_id = u.id
         INNER JOIN iam_roles r ON r.id = ur.role_id
         WHERE u.username = $1
           AND u.is_enabled = TRUE
         ORDER BY r.role_key ASC",
    )
    .bind(username)
    .fetch_all(db)
    .await
    .map_err(AppError::from)
}

async fn count_topology_nodes(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<i64> {
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM assets a WHERE 1=1");
    append_asset_scope_filters(&mut qb, site, department);

    qb.build_query_scalar()
        .fetch_one(db)
        .await
        .map_err(AppError::from)
}

async fn fetch_topology_nodes(
    db: &sqlx::PgPool,
    site: Option<&str>,
    department: Option<&str>,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<TopologyMapNode>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            a.id,
            a.name,
            a.asset_class,
            a.status,
            a.site,
            a.department,
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
    qb.push(" ORDER BY a.id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<TopologyMapNodeRow> = qb
        .build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(|row| TopologyMapNode {
            id: row.id,
            name: row.name,
            asset_class: row.asset_class,
            status: row.status,
            site: row.site,
            department: row.department,
            monitoring_status: row.monitoring_status.clone(),
            latest_job_status: row.latest_job_status.clone(),
            health: resolve_health_status(
                row.monitoring_status.as_deref(),
                row.latest_job_status.as_deref(),
            )
            .to_string(),
        })
        .collect())
}

async fn fetch_topology_edges(
    db: &sqlx::PgPool,
    node_ids: &HashSet<i64>,
) -> AppResult<Vec<TopologyMapEdge>> {
    if node_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut sorted_ids = node_ids.iter().copied().collect::<Vec<_>>();
    sorted_ids.sort_unstable();

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, src_asset_id, dst_asset_id, relation_type, source
         FROM asset_relations
         WHERE relation_type IN (",
    );
    {
        let mut separated = qb.separated(", ");
        for relation_type in TOPOLOGY_RELATION_TYPES {
            separated.push_bind(relation_type);
        }
        separated.push_unseparated(")");
    }

    qb.push(" AND src_asset_id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in &sorted_ids {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");
    }

    qb.push(" AND dst_asset_id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in &sorted_ids {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");
    }

    qb.push(" ORDER BY id ASC");

    let rows: Vec<TopologyMapEdgeRow> = qb
        .build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(|row| TopologyMapEdge {
            id: row.id,
            src_asset_id: row.src_asset_id,
            dst_asset_id: row.dst_asset_id,
            relation_type: row.relation_type,
            source: row.source,
        })
        .collect())
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

fn resolve_health_status(
    monitoring_status: Option<&str>,
    latest_job_status: Option<&str>,
) -> &'static str {
    let status = monitoring_status.or(latest_job_status).unwrap_or("unknown");
    match status.trim().to_ascii_lowercase().as_str() {
        "success" => "healthy",
        "pending" | "running" => "warning",
        "failed" | "dead_letter" => "critical",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::{
        build_edge_diagnostics_quick_actions, enforce_scope_access, merge_scope_filter,
        normalize_diagnostics_window_minutes, parse_scope_key, resolve_health_status,
    };

    #[test]
    fn parses_scope_key_variants() {
        let global = parse_scope_key("global").expect("global");
        assert!(global.0.is_none());
        assert!(global.1.is_none());

        let site = parse_scope_key("site:dc-a").expect("site");
        assert_eq!(site.0.as_deref(), Some("dc-a"));
        assert!(site.1.is_none());

        let department = parse_scope_key("department:platform").expect("department");
        assert!(department.0.is_none());
        assert_eq!(department.1.as_deref(), Some("platform"));
    }

    #[test]
    fn rejects_unknown_scope_key() {
        assert!(parse_scope_key("region:cn-north-1").is_err());
    }

    #[test]
    fn merge_scope_filter_detects_conflict() {
        let merged = merge_scope_filter(Some("dc-a".to_string()), Some("dc-a".to_string()), "site")
            .expect("same scope");
        assert_eq!(merged.as_deref(), Some("dc-a"));
        assert!(
            merge_scope_filter(Some("dc-a".to_string()), Some("dc-b".to_string()), "site").is_err()
        );
    }

    #[test]
    fn non_admin_must_supply_scope() {
        let mut site = None;
        let mut department = None;
        let headers = HeaderMap::new();
        let roles = vec!["viewer".to_string()];
        assert!(enforce_scope_access(&roles, &headers, &mut site, &mut department).is_err());
    }

    #[test]
    fn scope_headers_are_enforced_for_non_admin() {
        let mut site = Some("dc-b".to_string());
        let mut department = Some("platform".to_string());
        let mut headers = HeaderMap::new();
        headers.insert("x-auth-site", HeaderValue::from_static("dc-a"));
        let roles = vec!["operator".to_string()];
        assert!(enforce_scope_access(&roles, &headers, &mut site, &mut department).is_err());
    }

    #[test]
    fn health_status_mapping_is_stable() {
        assert_eq!(resolve_health_status(Some("success"), None), "healthy");
        assert_eq!(resolve_health_status(Some("running"), None), "warning");
        assert_eq!(resolve_health_status(Some("dead_letter"), None), "critical");
        assert_eq!(resolve_health_status(Some("skipped"), None), "unknown");
        assert_eq!(resolve_health_status(None, Some("failed")), "critical");
    }

    #[test]
    fn diagnostics_window_range_is_enforced() {
        assert_eq!(
            normalize_diagnostics_window_minutes(None).expect("default"),
            120
        );
        assert!(normalize_diagnostics_window_minutes(Some(10)).is_err());
        assert!(normalize_diagnostics_window_minutes(Some(2_000)).is_err());
    }

    #[test]
    fn diagnostics_quick_actions_include_playbook_and_ticket_links() {
        let actions = build_edge_diagnostics_quick_actions(9);
        assert!(actions.iter().any(|item| item.key == "open-ticket-center"));
        assert!(
            actions
                .iter()
                .any(|item| item.key == "run-diagnostics-playbook")
        );
    }
}
