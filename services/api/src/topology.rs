use std::collections::HashSet;

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
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const AUTH_SITE_HEADER: &str = "x-auth-site";
const AUTH_DEPARTMENT_HEADER: &str = "x-auth-department";
const TOPOLOGY_LIMIT_DEFAULT: u32 = 200;
const TOPOLOGY_OFFSET_DEFAULT: u32 = 0;
const TOPOLOGY_RELATION_TYPES: [&str; 4] = ["contains", "depends_on", "runs_service", "owned_by"];

pub fn routes() -> Router<AppState> {
    Router::new().route("/maps/{scope}", get(get_topology_map))
}

#[derive(Debug, Deserialize, Default)]
struct TopologyMapQuery {
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
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

    use super::{enforce_scope_access, merge_scope_filter, parse_scope_key, resolve_health_status};

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
}
