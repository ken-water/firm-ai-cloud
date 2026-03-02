use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashSet;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/relations", get(list_relations).post(create_relation))
        .route("/assets/{asset_id}/graph", get(get_asset_graph))
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

async fn create_relation(
    State(state): State<AppState>,
    Json(payload): Json<CreateRelationRequest>,
) -> AppResult<Json<AssetRelation>> {
    validate_self_loop(
        payload.src_asset_id,
        payload.dst_asset_id,
        &payload.relation_type,
    )?;

    let relation_type = normalize_relation_type(payload.relation_type)?;
    let source = normalize_source(payload.source);

    ensure_asset_exists(&state.db, payload.src_asset_id).await?;
    ensure_asset_exists(&state.db, payload.dst_asset_id).await?;

    let relation: AssetRelation = sqlx::query_as(
        "INSERT INTO asset_relations (src_asset_id, dst_asset_id, relation_type, source)
         VALUES ($1, $2, $3, $4)
         RETURNING id, src_asset_id, dst_asset_id, relation_type, source, created_at, updated_at",
    )
    .bind(payload.src_asset_id)
    .bind(payload.dst_asset_id)
    .bind(relation_type)
    .bind(source)
    .fetch_one(&state.db)
    .await
    .map_err(map_relation_conflict)?;

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
    Ok(trimmed)
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
