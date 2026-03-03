use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::{HashMap, HashSet};

use crate::state::AppState;
use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/relations", get(list_relations).post(create_relation))
        .route("/assets/{asset_id}/graph", get(get_asset_graph))
        .route("/assets/{asset_id}/impact", get(get_asset_impact))
        .route(
            "/relations/{relation_id}",
            axum::routing::delete(delete_relation),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AssetRelation {
    id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateRelationRequest {
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListRelationsQuery {
    asset_id: i64,
}

#[derive(Debug, Deserialize, Default)]
struct AssetImpactQuery {
    direction: Option<String>,
    depth: Option<u32>,
    relation_types: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeleteRelationResponse {
    id: i64,
    deleted: bool,
}

#[derive(Debug, Serialize)]
struct AssetGraphResponse {
    root_asset_id: i64,
    nodes: Vec<AssetGraphNode>,
    edges: Vec<AssetGraphEdge>,
}

#[derive(Debug, Serialize)]
struct AssetImpactResponse {
    root_asset_id: i64,
    direction: String,
    depth_limit: u32,
    relation_types: Vec<String>,
    nodes: Vec<ImpactNode>,
    edges: Vec<ImpactEdge>,
    affected_business_services: Vec<AssetGraphNode>,
    affected_owners: Vec<AssetGraphNode>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AssetGraphNode {
    id: i64,
    name: String,
    asset_class: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct AssetGraphEdge {
    id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct ImpactNode {
    id: i64,
    name: String,
    asset_class: String,
    status: String,
    depth: u32,
}

#[derive(Debug, Serialize, Clone)]
struct ImpactEdge {
    id: i64,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: String,
    source: String,
    direction: String,
    depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum TraversalDirection {
    Downstream,
    Upstream,
}

#[derive(Debug, Clone, Copy)]
enum ImpactDirectionMode {
    Downstream,
    Upstream,
    Both,
}

async fn create_relation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRelationRequest>,
) -> AppResult<Json<AssetRelation>> {
    let relation_type = normalize_relation_type(payload.relation_type)?;
    validate_self_loop(payload.src_asset_id, payload.dst_asset_id, &relation_type)?;
    let source = normalize_source(payload.source);

    ensure_asset_exists(&state.db, payload.src_asset_id).await?;
    ensure_asset_exists(&state.db, payload.dst_asset_id).await?;
    validate_hierarchy_relation(
        &state.db,
        payload.src_asset_id,
        payload.dst_asset_id,
        relation_type.as_str(),
    )
    .await?;

    let relation: AssetRelation = sqlx::query_as(
        "INSERT INTO asset_relations (src_asset_id, dst_asset_id, relation_type, source)
         VALUES ($1, $2, $3, $4)
         RETURNING id, src_asset_id, dst_asset_id, relation_type, source, created_at, updated_at",
    )
    .bind(payload.src_asset_id)
    .bind(payload.dst_asset_id)
    .bind(relation_type.clone())
    .bind(source)
    .fetch_one(&state.db)
    .await
    .map_err(map_relation_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.relation.create",
        "relation",
        Some(relation.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "src_asset_id": relation.src_asset_id,
            "dst_asset_id": relation.dst_asset_id,
            "relation_type": &relation.relation_type
        }),
    )
    .await;

    Ok(Json(relation))
}

async fn list_relations(
    State(state): State<AppState>,
    Query(query): Query<ListRelationsQuery>,
) -> AppResult<Json<Vec<AssetRelation>>> {
    ensure_asset_exists(&state.db, query.asset_id).await?;

    let items: Vec<AssetRelation> = sqlx::query_as(
        "SELECT id, src_asset_id, dst_asset_id, relation_type, source, created_at, updated_at
         FROM asset_relations
         WHERE src_asset_id = $1 OR dst_asset_id = $1
         ORDER BY id DESC",
    )
    .bind(query.asset_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(items))
}

async fn delete_relation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(relation_id): Path<i64>,
) -> AppResult<Json<DeleteRelationResponse>> {
    let deleted: Option<i64> = sqlx::query_scalar(
        "DELETE FROM asset_relations
         WHERE id = $1
         RETURNING id",
    )
    .bind(relation_id)
    .fetch_optional(&state.db)
    .await?;

    if deleted.is_none() {
        return Err(AppError::NotFound(format!(
            "relation {relation_id} not found"
        )));
    }

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.relation.delete",
        "relation",
        Some(relation_id.to_string()),
        "success",
        None,
        serde_json::json!({}),
    )
    .await;

    Ok(Json(DeleteRelationResponse {
        id: relation_id,
        deleted: true,
    }))
}

async fn get_asset_graph(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
) -> AppResult<Json<AssetGraphResponse>> {
    let root_node: Option<AssetGraphNode> = sqlx::query_as(
        "SELECT id, name, asset_class, status
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    let root_node =
        root_node.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))?;

    let relations: Vec<AssetRelation> = sqlx::query_as(
        "SELECT id, src_asset_id, dst_asset_id, relation_type, source, created_at, updated_at
         FROM asset_relations
         WHERE src_asset_id = $1 OR dst_asset_id = $1
         ORDER BY id DESC",
    )
    .bind(asset_id)
    .fetch_all(&state.db)
    .await?;

    let mut asset_ids = HashSet::new();
    asset_ids.insert(asset_id);
    for relation in &relations {
        asset_ids.insert(relation.src_asset_id);
        asset_ids.insert(relation.dst_asset_id);
    }

    let mut nodes = fetch_graph_nodes(&state.db, asset_ids).await?;
    if !nodes.iter().any(|node| node.id == root_node.id) {
        nodes.push(root_node);
    }
    nodes.sort_by_key(|node| node.id);

    let edges = relations
        .into_iter()
        .map(|item| AssetGraphEdge {
            id: item.id,
            src_asset_id: item.src_asset_id,
            dst_asset_id: item.dst_asset_id,
            relation_type: item.relation_type,
            source: item.source,
        })
        .collect();

    Ok(Json(AssetGraphResponse {
        root_asset_id: asset_id,
        nodes,
        edges,
    }))
}

async fn get_asset_impact(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
    Query(query): Query<AssetImpactQuery>,
) -> AppResult<Json<AssetImpactResponse>> {
    let root_node: Option<AssetGraphNode> = sqlx::query_as(
        "SELECT id, name, asset_class, status
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;
    let root_node =
        root_node.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))?;

    let direction_mode = parse_impact_direction(query.direction)?;
    let depth_limit = query.depth.unwrap_or(3).clamp(1, 8);
    let relation_types = parse_relation_type_filter(query.relation_types)?;

    let mut node_depths: HashMap<i64, u32> = HashMap::new();
    node_depths.insert(asset_id, 0);
    let mut edges_by_key: HashMap<(i64, String), ImpactEdge> = HashMap::new();

    if matches!(
        direction_mode,
        ImpactDirectionMode::Downstream | ImpactDirectionMode::Both
    ) {
        let (depths, edges) = traverse_impact(
            &state.db,
            asset_id,
            depth_limit,
            relation_types.as_slice(),
            TraversalDirection::Downstream,
        )
        .await?;
        merge_impact(&mut node_depths, &mut edges_by_key, depths, edges);
    }

    if matches!(
        direction_mode,
        ImpactDirectionMode::Upstream | ImpactDirectionMode::Both
    ) {
        let (depths, edges) = traverse_impact(
            &state.db,
            asset_id,
            depth_limit,
            relation_types.as_slice(),
            TraversalDirection::Upstream,
        )
        .await?;
        merge_impact(&mut node_depths, &mut edges_by_key, depths, edges);
    }

    let mut node_ids = HashSet::new();
    for id in node_depths.keys() {
        node_ids.insert(*id);
    }
    let mut nodes = fetch_graph_nodes(&state.db, node_ids).await?;
    if !nodes.iter().any(|item| item.id == root_node.id) {
        nodes.push(root_node);
    }

    let mut impact_nodes = nodes
        .into_iter()
        .map(|node| ImpactNode {
            depth: *node_depths.get(&node.id).unwrap_or(&0),
            id: node.id,
            name: node.name,
            asset_class: node.asset_class,
            status: node.status,
        })
        .collect::<Vec<_>>();
    impact_nodes.sort_by_key(|node| (node.depth, node.id));

    let mut edges = edges_by_key.into_values().collect::<Vec<_>>();
    edges.sort_by_key(|edge| (edge.depth, edge.id, edge.direction.clone()));

    let mut business_ids = HashSet::new();
    let mut owner_ids = HashSet::new();
    for edge in &edges {
        if edge.relation_type == "runs_service" {
            business_ids.insert(edge.dst_asset_id);
        }
        if edge.relation_type == "owned_by" {
            owner_ids.insert(edge.dst_asset_id);
        }
    }

    let mut affected_business_services = fetch_graph_nodes(&state.db, business_ids).await?;
    affected_business_services.sort_by_key(|node| node.id);
    let mut affected_owners = fetch_graph_nodes(&state.db, owner_ids).await?;
    affected_owners.sort_by_key(|node| node.id);

    Ok(Json(AssetImpactResponse {
        root_asset_id: asset_id,
        direction: direction_mode.as_str().to_string(),
        depth_limit,
        relation_types,
        nodes: impact_nodes,
        edges,
        affected_business_services,
        affected_owners,
    }))
}

async fn traverse_impact(
    db: &PgPool,
    root_asset_id: i64,
    depth_limit: u32,
    relation_types: &[String],
    direction: TraversalDirection,
) -> AppResult<(HashMap<i64, u32>, Vec<ImpactEdge>)> {
    let mut node_depths = HashMap::new();
    let mut visited_nodes = HashSet::new();
    let mut frontier = HashSet::new();

    node_depths.insert(root_asset_id, 0_u32);
    visited_nodes.insert(root_asset_id);
    frontier.insert(root_asset_id);

    let mut edges_by_id: HashMap<i64, ImpactEdge> = HashMap::new();

    for depth in 1..=depth_limit {
        if frontier.is_empty() {
            break;
        }

        let relations =
            fetch_relations_by_frontier(db, &frontier, direction, relation_types).await?;
        let mut next_frontier = HashSet::new();

        for relation in relations {
            let next_node = match direction {
                TraversalDirection::Downstream => relation.dst_asset_id,
                TraversalDirection::Upstream => relation.src_asset_id,
            };

            let direction_label = match direction {
                TraversalDirection::Downstream => "downstream",
                TraversalDirection::Upstream => "upstream",
            };

            let edge = ImpactEdge {
                id: relation.id,
                src_asset_id: relation.src_asset_id,
                dst_asset_id: relation.dst_asset_id,
                relation_type: relation.relation_type,
                source: relation.source,
                direction: direction_label.to_string(),
                depth,
            };
            edges_by_id
                .entry(edge.id)
                .and_modify(|existing| {
                    if edge.depth < existing.depth {
                        *existing = edge.clone();
                    }
                })
                .or_insert(edge);

            node_depths
                .entry(next_node)
                .and_modify(|existing| {
                    if depth < *existing {
                        *existing = depth;
                    }
                })
                .or_insert(depth);

            if !visited_nodes.contains(&next_node) {
                visited_nodes.insert(next_node);
                next_frontier.insert(next_node);
            }
        }

        frontier = next_frontier;
    }

    Ok((node_depths, edges_by_id.into_values().collect()))
}

async fn fetch_relations_by_frontier(
    db: &PgPool,
    frontier: &HashSet<i64>,
    direction: TraversalDirection,
    relation_types: &[String],
) -> AppResult<Vec<AssetRelation>> {
    if frontier.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, src_asset_id, dst_asset_id, relation_type, source, created_at, updated_at
         FROM asset_relations
         WHERE ",
    );

    match direction {
        TraversalDirection::Downstream => qb.push("src_asset_id IN ("),
        TraversalDirection::Upstream => qb.push("dst_asset_id IN ("),
    };
    let mut separated = qb.separated(", ");
    for id in frontier {
        separated.push_bind(*id);
    }
    separated.push_unseparated(")");
    drop(separated);

    if !relation_types.is_empty() {
        qb.push(" AND relation_type IN (");
        let mut separated_types = qb.separated(", ");
        for relation_type in relation_types {
            separated_types.push_bind(relation_type);
        }
        separated_types.push_unseparated(")");
        drop(separated_types);
    }

    qb.push(" ORDER BY id ASC");
    qb.build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)
}

fn merge_impact(
    node_depths: &mut HashMap<i64, u32>,
    edges_by_key: &mut HashMap<(i64, String), ImpactEdge>,
    add_depths: HashMap<i64, u32>,
    add_edges: Vec<ImpactEdge>,
) {
    for (node_id, depth) in add_depths {
        node_depths
            .entry(node_id)
            .and_modify(|existing| {
                if depth < *existing {
                    *existing = depth;
                }
            })
            .or_insert(depth);
    }

    for edge in add_edges {
        let key = (edge.id, edge.direction.clone());
        edges_by_key
            .entry(key)
            .and_modify(|existing| {
                if edge.depth < existing.depth {
                    *existing = edge.clone();
                }
            })
            .or_insert(edge);
    }
}

fn validate_self_loop(src_asset_id: i64, dst_asset_id: i64, relation_type: &str) -> AppResult<()> {
    if src_asset_id != dst_asset_id {
        return Ok(());
    }

    let normalized = relation_type.trim().to_ascii_lowercase();
    let allowed_self_loop = ["self", "loopback"];
    if allowed_self_loop.contains(&normalized.as_str()) {
        return Ok(());
    }

    Err(AppError::Validation(
        "self-loop relation is not allowed for this relation_type".to_string(),
    ))
}

async fn validate_hierarchy_relation(
    db: &PgPool,
    src_asset_id: i64,
    dst_asset_id: i64,
    relation_type: &str,
) -> AppResult<()> {
    if relation_type != "contains" {
        return Ok(());
    }

    if src_asset_id == dst_asset_id {
        return Err(AppError::Validation(
            "contains relation cannot reference the same asset as parent and child".to_string(),
        ));
    }

    let existing_parent: Option<i64> = sqlx::query_scalar(
        "SELECT src_asset_id
         FROM asset_relations
         WHERE relation_type = 'contains'
           AND dst_asset_id = $1
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(dst_asset_id)
    .fetch_optional(db)
    .await?;
    if let Some(existing_parent) = existing_parent {
        if existing_parent != src_asset_id {
            return Err(AppError::Validation(format!(
                "contains relation requires single parent per child; child {} already belongs to parent {}",
                dst_asset_id, existing_parent
            )));
        }
    }

    let creates_cycle: bool = sqlx::query_scalar(
        "WITH RECURSIVE descendants AS (
            SELECT dst_asset_id AS node_id
            FROM asset_relations
            WHERE relation_type = 'contains'
              AND src_asset_id = $1
            UNION
            SELECT r.dst_asset_id
            FROM asset_relations r
            INNER JOIN descendants d ON d.node_id = r.src_asset_id
            WHERE r.relation_type = 'contains'
        )
        SELECT EXISTS(SELECT 1 FROM descendants WHERE node_id = $2)",
    )
    .bind(dst_asset_id)
    .bind(src_asset_id)
    .fetch_one(db)
    .await?;
    if creates_cycle {
        return Err(AppError::Validation(
            "contains relation would create a hierarchy cycle".to_string(),
        ));
    }

    Ok(())
}

fn normalize_relation_type(value: String) -> AppResult<String> {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "relation_type is required".to_string(),
        ));
    }
    if trimmed.len() > 64 {
        return Err(AppError::Validation(
            "relation_type length must be <= 64".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "relation_type can only contain lowercase letters, numbers, '-', '_'".to_string(),
        ));
    }

    let canonical = match trimmed.as_str() {
        "contains" | "hosts" | "host" | "parent_of" => "contains",
        "depends_on" | "dependency" | "requires" => "depends_on",
        "runs_service" | "serves" | "service_for" => "runs_service",
        "owned_by" | "owner" | "managed_by" => "owned_by",
        _ => trimmed.as_str(),
    };
    Ok(canonical.to_string())
}

fn normalize_source(value: Option<String>) -> String {
    value
        .and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_lowercase())
            }
        })
        .unwrap_or_else(|| "manual".to_string())
}

fn parse_impact_direction(value: Option<String>) -> AppResult<ImpactDirectionMode> {
    let normalized = value
        .and_then(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_lowercase())
            }
        })
        .unwrap_or_else(|| "both".to_string());

    match normalized.as_str() {
        "downstream" => Ok(ImpactDirectionMode::Downstream),
        "upstream" => Ok(ImpactDirectionMode::Upstream),
        "both" => Ok(ImpactDirectionMode::Both),
        _ => Err(AppError::Validation(
            "direction must be one of: downstream, upstream, both".to_string(),
        )),
    }
}

fn parse_relation_type_filter(value: Option<String>) -> AppResult<Vec<String>> {
    let Some(raw) = value else {
        return Ok(default_impact_relation_types());
    };

    let mut normalized = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let relation = normalize_relation_type(part.to_string())?;
        if !normalized.contains(&relation) {
            normalized.push(relation);
        }
    }

    if normalized.is_empty() {
        return Ok(default_impact_relation_types());
    }

    Ok(normalized)
}

fn default_impact_relation_types() -> Vec<String> {
    vec![
        "contains".to_string(),
        "depends_on".to_string(),
        "runs_service".to_string(),
        "owned_by".to_string(),
    ]
}

impl ImpactDirectionMode {
    fn as_str(self) -> &'static str {
        match self {
            ImpactDirectionMode::Downstream => "downstream",
            ImpactDirectionMode::Upstream => "upstream",
            ImpactDirectionMode::Both => "both",
        }
    }
}

async fn ensure_asset_exists(db: &PgPool, asset_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assets WHERE id = $1)")
        .bind(asset_id)
        .fetch_one(db)
        .await?;

    if !exists {
        return Err(AppError::Validation(format!(
            "asset {asset_id} does not exist"
        )));
    }
    Ok(())
}

fn map_relation_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            if db_err.constraint().as_deref() == Some("uq_asset_relations_contains_single_parent") {
                return AppError::Validation(
                    "contains relation requires single parent per child".to_string(),
                );
            }
            return AppError::Validation("relation already exists".to_string());
        }
    }
    AppError::Database(err)
}

async fn fetch_graph_nodes(db: &PgPool, ids: HashSet<i64>) -> AppResult<Vec<AssetGraphNode>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT id, name, asset_class, status FROM assets WHERE id IN (");
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    drop(separated);

    qb.build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::{default_impact_relation_types, normalize_relation_type, parse_impact_direction};

    #[test]
    fn normalizes_standard_relation_type_aliases() {
        assert_eq!(
            normalize_relation_type("hosts".to_string()).expect("hosts alias"),
            "contains"
        );
        assert_eq!(
            normalize_relation_type("dependency".to_string()).expect("dependency alias"),
            "depends_on"
        );
        assert_eq!(
            normalize_relation_type("serves".to_string()).expect("serves alias"),
            "runs_service"
        );
        assert_eq!(
            normalize_relation_type("managed_by".to_string()).expect("managed_by alias"),
            "owned_by"
        );
    }

    #[test]
    fn parses_impact_direction_with_default() {
        assert!(parse_impact_direction(None).is_ok());
        assert!(parse_impact_direction(Some("downstream".to_string())).is_ok());
        assert!(parse_impact_direction(Some("upstream".to_string())).is_ok());
        assert!(parse_impact_direction(Some("both".to_string())).is_ok());
        assert!(parse_impact_direction(Some("invalid".to_string())).is_err());
    }

    #[test]
    fn default_impact_relation_types_are_stable() {
        assert_eq!(
            default_impact_relation_types(),
            vec![
                "contains".to_string(),
                "depends_on".to_string(),
                "runs_service".to_string(),
                "owned_by".to_string()
            ]
        );
    }
}
