use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, QueryBuilder};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list_assets).post(create_asset))
        .route("/assets/{asset_id}", get(get_asset))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct Asset {
    id: i64,
    asset_class: String,
    name: String,
    hostname: Option<String>,
    ip: Option<String>,
    status: String,
    site: Option<String>,
    department: Option<String>,
    owner: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
struct ListAssetsQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    q: Option<String>,
    status: Option<String>,
    class: Option<String>,
}

#[derive(Debug)]
struct AssetFilters {
    limit: i64,
    offset: i64,
    status: Option<String>,
    class: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListAssetsResponse {
    items: Vec<Asset>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Deserialize)]
struct CreateAssetRequest {
    asset_class: String,
    name: String,
    hostname: Option<String>,
    ip: Option<String>,
    status: Option<String>,
    site: Option<String>,
    department: Option<String>,
    owner: Option<String>,
}

async fn list_assets(
    State(state): State<AppState>,
    Query(query): Query<ListAssetsQuery>,
) -> AppResult<Json<ListAssetsResponse>> {
    let filters = parse_filters(query);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM assets WHERE 1=1");
    append_asset_filters(&mut count_builder, &filters);
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, created_at, updated_at \
         FROM assets WHERE 1=1",
    );
    append_asset_filters(&mut list_builder, &filters);
    list_builder
        .push(" ORDER BY id DESC LIMIT ")
        .push_bind(filters.limit)
        .push(" OFFSET ")
        .push_bind(filters.offset);

    let items: Vec<Asset> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListAssetsResponse {
        items,
        total,
        limit: filters.limit as u32,
        offset: filters.offset as u32,
    }))
}

async fn create_asset(
    State(state): State<AppState>,
    Json(payload): Json<CreateAssetRequest>,
) -> AppResult<Json<Asset>> {
    let asset_class = required_field("asset_class", payload.asset_class)?;
    let name = required_field("name", payload.name)?;
    let status = optional_or_default(payload.status, "active");

    let asset: Asset = sqlx::query_as(
        "INSERT INTO assets (asset_class, name, hostname, ip, status, site, department, owner)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, asset_class, name, hostname, ip, status, site, department, owner, created_at, updated_at",
    )
    .bind(asset_class)
    .bind(name)
    .bind(trim_optional(payload.hostname))
    .bind(trim_optional(payload.ip))
    .bind(status)
    .bind(trim_optional(payload.site))
    .bind(trim_optional(payload.department))
    .bind(trim_optional(payload.owner))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(asset))
}

async fn get_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
) -> AppResult<Json<Asset>> {
    let asset: Option<Asset> = sqlx::query_as(
        "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, created_at, updated_at
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    let asset = asset.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))?;
    Ok(Json(asset))
}

fn parse_filters(query: ListAssetsQuery) -> AssetFilters {
    AssetFilters {
        limit: query.limit.unwrap_or(20).min(200) as i64,
        offset: query.offset.unwrap_or(0) as i64,
        status: trim_optional(query.status),
        class: trim_optional(query.class),
        keyword: trim_optional(query.q),
    }
}

fn append_asset_filters(builder: &mut QueryBuilder<Postgres>, filters: &AssetFilters) {
    if let Some(status) = &filters.status {
        builder.push(" AND status = ").push_bind(status.clone());
    }

    if let Some(asset_class) = &filters.class {
        builder
            .push(" AND asset_class = ")
            .push_bind(asset_class.clone());
    }

    if let Some(keyword) = &filters.keyword {
        let like = format!("%{keyword}%");
        builder
            .push(" AND (name ILIKE ")
            .push_bind(like.clone())
            .push(" OR COALESCE(hostname, '') ILIKE ")
            .push_bind(like.clone())
            .push(" OR COALESCE(ip, '') ILIKE ")
            .push_bind(like)
            .push(")");
    }
}

fn required_field(field: &str, value: String) -> AppResult<String> {
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

fn optional_or_default(value: Option<String>, default: &str) -> String {
    trim_optional(value).unwrap_or_else(|| default.to_string())
}
