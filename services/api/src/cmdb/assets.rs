use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Postgres, QueryBuilder};

use crate::state::AppState;
use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
};

use super::field_definitions::{
    FieldDefinitionRecord, fetch_enabled_definitions, validate_custom_field_value,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list_assets).post(create_asset))
        .route("/assets/by-code/{code}", get(get_asset_by_code))
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
    qr_code: Option<String>,
    barcode: Option<String>,
    custom_fields: Value,
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

#[derive(Debug, Deserialize, Default)]
struct ScanModeQuery {
    mode: Option<String>,
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
    qr_code: Option<String>,
    barcode: Option<String>,
    custom_fields: Option<Value>,
}

#[derive(Debug, Clone, Copy)]
enum ScanMode {
    Auto,
    Qr,
    Barcode,
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
        "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at \
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
    headers: HeaderMap,
    Json(payload): Json<CreateAssetRequest>,
) -> AppResult<Json<Asset>> {
    let asset_class = required_field("asset_class", payload.asset_class)?;
    let name = required_field("name", payload.name)?;
    let status = optional_or_default(payload.status, "active");
    let qr_code = trim_optional(payload.qr_code);
    let barcode = trim_optional(payload.barcode);

    if let Some(code) = qr_code.as_deref() {
        ensure_unique_code(&state, "qr_code", code).await?;
    }
    if let Some(code) = barcode.as_deref() {
        ensure_unique_code(&state, "barcode", code).await?;
    }

    let custom_fields = normalize_custom_fields(payload.custom_fields)?;
    let definitions = fetch_enabled_definitions(&state.db).await?;
    validate_custom_fields(&definitions, &custom_fields)?;

    let asset: Asset = sqlx::query_as(
        "INSERT INTO assets (asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at",
    )
    .bind(asset_class)
    .bind(name)
    .bind(trim_optional(payload.hostname))
    .bind(trim_optional(payload.ip))
    .bind(status)
    .bind(trim_optional(payload.site))
    .bind(trim_optional(payload.department))
    .bind(trim_optional(payload.owner))
    .bind(qr_code)
    .bind(barcode)
    .bind(Value::Object(custom_fields))
    .fetch_one(&state.db)
    .await
    .map_err(map_asset_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.asset.create",
        "asset",
        Some(asset.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "asset_class": &asset.asset_class,
            "name": &asset.name
        }),
    )
    .await;

    Ok(Json(asset))
}

async fn get_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
) -> AppResult<Json<Asset>> {
    let asset: Option<Asset> = sqlx::query_as(
        "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    let asset = asset.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))?;
    Ok(Json(asset))
}

async fn get_asset_by_code(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<ScanModeQuery>,
) -> AppResult<Json<Asset>> {
    let code = required_field("code", code)?;
    let mode = parse_scan_mode(query.mode)?;

    let sql = match mode {
        ScanMode::Auto => {
            "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at
             FROM assets
             WHERE qr_code = $1 OR barcode = $1
             ORDER BY CASE WHEN qr_code = $1 THEN 0 ELSE 1 END, id DESC
             LIMIT 1"
        }
        ScanMode::Qr => {
            "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at
             FROM assets
             WHERE qr_code = $1
             ORDER BY id DESC
             LIMIT 1"
        }
        ScanMode::Barcode => {
            "SELECT id, asset_class, name, hostname, ip, status, site, department, owner, qr_code, barcode, custom_fields, created_at, updated_at
             FROM assets
             WHERE barcode = $1
             ORDER BY id DESC
             LIMIT 1"
        }
    };

    let asset: Option<Asset> = sqlx::query_as(sql)
        .bind(code)
        .fetch_optional(&state.db)
        .await?;

    let asset = asset.ok_or_else(|| AppError::NotFound("asset not found by code".to_string()))?;
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
            .push_bind(like.clone())
            .push(" OR COALESCE(qr_code, '') ILIKE ")
            .push_bind(like.clone())
            .push(" OR COALESCE(barcode, '') ILIKE ")
            .push_bind(like)
            .push(")");
    }
}

fn parse_scan_mode(mode: Option<String>) -> AppResult<ScanMode> {
    match mode
        .unwrap_or_else(|| "auto".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "auto" => Ok(ScanMode::Auto),
        "qr" => Ok(ScanMode::Qr),
        "barcode" => Ok(ScanMode::Barcode),
        _ => Err(AppError::Validation(
            "mode must be one of: auto, qr, barcode".to_string(),
        )),
    }
}

fn normalize_custom_fields(value: Option<Value>) -> AppResult<Map<String, Value>> {
    let Some(value) = value else {
        return Ok(Map::new());
    };

    let map = value
        .as_object()
        .ok_or_else(|| AppError::Validation("custom_fields must be a JSON object".to_string()))?;

    let mut normalized = Map::new();
    for (key, value) in map {
        let normalized_key = key.trim().to_ascii_lowercase();
        if normalized_key.is_empty() {
            return Err(AppError::Validation(
                "custom_fields cannot contain empty key".to_string(),
            ));
        }
        if value.is_null() {
            continue;
        }
        if normalized
            .insert(normalized_key.clone(), value.clone())
            .is_some()
        {
            return Err(AppError::Validation(format!(
                "custom field key '{}' is duplicated after normalization",
                normalized_key
            )));
        }
    }

    Ok(normalized)
}

fn validate_custom_fields(
    definitions: &HashMap<String, FieldDefinitionRecord>,
    custom_fields: &Map<String, Value>,
) -> AppResult<()> {
    for key in custom_fields.keys() {
        if !definitions.contains_key(key) {
            return Err(AppError::Validation(format!(
                "custom field '{}' is not defined or disabled",
                key
            )));
        }
    }

    for definition in definitions.values() {
        match custom_fields.get(&definition.field_key) {
            Some(value) => validate_custom_field_value(definition, value)?,
            None if definition.required => {
                return Err(AppError::Validation(format!(
                    "custom field '{}' is required",
                    definition.field_key
                )));
            }
            None => {}
        }
    }

    Ok(())
}

async fn ensure_unique_code(state: &AppState, column: &str, code: &str) -> AppResult<()> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM assets WHERE {column} = $1)");
    let exists: bool = sqlx::query_scalar(&sql)
        .bind(code)
        .fetch_one(&state.db)
        .await?;
    if exists {
        return Err(AppError::Validation(format!("{column} already exists")));
    }
    Ok(())
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

fn map_asset_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            let message = match db_err.constraint() {
                Some(name) if name.contains("qr_code") => "qr_code already exists",
                Some(name) if name.contains("barcode") => "barcode already exists",
                _ => "asset unique constraint violated",
            };
            return AppError::Validation(message.to_string());
        }
    }
    AppError::Database(err)
}
