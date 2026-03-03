use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
    state::AppState,
};

const STATUS_IDLE: &str = "idle";
const STATUS_ONBOARDING: &str = "onboarding";
const STATUS_OPERATIONAL: &str = "operational";
const STATUS_MAINTENANCE: &str = "maintenance";
const STATUS_RETIRED: &str = "retired";

const OWNER_TYPE_TEAM: &str = "team";
const OWNER_TYPE_USER: &str = "user";
const OWNER_TYPE_GROUP: &str = "group";
const OWNER_TYPE_EXTERNAL: &str = "external";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/assets/{asset_id}/bindings",
            get(get_asset_bindings).put(upsert_asset_bindings),
        )
        .route(
            "/assets/{asset_id}/lifecycle",
            axum::routing::post(transition_asset_lifecycle),
        )
}

#[derive(Debug, Serialize)]
struct AssetBindingsResponse {
    asset_id: i64,
    departments: Vec<String>,
    business_services: Vec<String>,
    owners: Vec<AssetOwnerBinding>,
    readiness: AssetOperationalReadiness,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AssetOwnerBinding {
    owner_type: String,
    owner_ref: String,
}

#[derive(Debug, Serialize)]
struct AssetOperationalReadiness {
    department_count: i64,
    business_service_count: i64,
    owner_count: i64,
    can_transition_operational: bool,
    missing: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertAssetBindingsRequest {
    departments: Option<Vec<String>>,
    business_services: Option<Vec<String>>,
    owners: Option<Vec<UpsertAssetOwnerBinding>>,
}

#[derive(Debug, Deserialize)]
struct UpsertAssetOwnerBinding {
    owner_type: String,
    owner_ref: String,
}

#[derive(Debug, Deserialize)]
struct TransitionAssetLifecycleRequest {
    status: String,
}

#[derive(Debug, Serialize)]
struct AssetLifecycleTransitionResponse {
    asset_id: i64,
    previous_status: String,
    status: String,
    readiness: AssetOperationalReadiness,
}

async fn get_asset_bindings(
    State(state): State<AppState>,
    Path(asset_id): Path<i64>,
) -> AppResult<Json<AssetBindingsResponse>> {
    ensure_asset_exists(&state.db, asset_id).await?;
    let response = load_asset_bindings_response(&state.db, asset_id).await?;
    Ok(Json(response))
}

async fn upsert_asset_bindings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(asset_id): Path<i64>,
    Json(payload): Json<UpsertAssetBindingsRequest>,
) -> AppResult<Json<AssetBindingsResponse>> {
    ensure_asset_exists(&state.db, asset_id).await?;

    let departments = normalize_string_bindings(payload.departments, "departments", 128)?;
    let business_services =
        normalize_string_bindings(payload.business_services, "business_services", 128)?;
    let owners = normalize_owner_bindings(payload.owners)?;

    let mut tx = state.db.begin().await?;

    if let Some(departments) = departments {
        sqlx::query("DELETE FROM asset_department_bindings WHERE asset_id = $1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await?;

        for department in departments {
            sqlx::query(
                "INSERT INTO asset_department_bindings (asset_id, department)
                 VALUES ($1, $2)",
            )
            .bind(asset_id)
            .bind(department)
            .execute(&mut *tx)
            .await?;
        }
    }

    if let Some(business_services) = business_services {
        sqlx::query("DELETE FROM asset_business_service_bindings WHERE asset_id = $1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await?;

        for business_service in business_services {
            sqlx::query(
                "INSERT INTO asset_business_service_bindings (asset_id, business_service)
                 VALUES ($1, $2)",
            )
            .bind(asset_id)
            .bind(business_service)
            .execute(&mut *tx)
            .await?;
        }
    }

    if let Some(owners) = owners {
        sqlx::query("DELETE FROM asset_owner_bindings WHERE asset_id = $1")
            .bind(asset_id)
            .execute(&mut *tx)
            .await?;

        for owner in owners {
            sqlx::query(
                "INSERT INTO asset_owner_bindings (asset_id, owner_type, owner_ref)
                 VALUES ($1, $2, $3)",
            )
            .bind(asset_id)
            .bind(owner.owner_type)
            .bind(owner.owner_ref)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    let response = load_asset_bindings_response(&state.db, asset_id).await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.asset.bindings.upsert",
        "asset",
        Some(asset_id.to_string()),
        "success",
        None,
        json!({
            "department_count": response.readiness.department_count,
            "business_service_count": response.readiness.business_service_count,
            "owner_count": response.readiness.owner_count
        }),
    )
    .await;

    Ok(Json(response))
}

async fn transition_asset_lifecycle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(asset_id): Path<i64>,
    Json(payload): Json<TransitionAssetLifecycleRequest>,
) -> AppResult<Json<AssetLifecycleTransitionResponse>> {
    let status = normalize_lifecycle_status(payload.status)?;

    let previous_status = get_asset_status(&state.db, asset_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))?;

    let readiness = evaluate_operational_readiness(&state.db, asset_id).await?;
    if status == STATUS_OPERATIONAL && !readiness.can_transition_operational {
        return Err(AppError::Validation(format!(
            "asset cannot transition to operational, missing requirements: {}",
            readiness.missing.join(", ")
        )));
    }

    sqlx::query(
        "UPDATE assets
         SET status = $2,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(asset_id)
    .bind(&status)
    .execute(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.asset.lifecycle.transition",
        "asset",
        Some(asset_id.to_string()),
        "success",
        None,
        json!({
            "previous_status": previous_status,
            "status": status
        }),
    )
    .await;

    Ok(Json(AssetLifecycleTransitionResponse {
        asset_id,
        previous_status,
        status,
        readiness,
    }))
}

async fn load_asset_bindings_response(
    db: &sqlx::PgPool,
    asset_id: i64,
) -> AppResult<AssetBindingsResponse> {
    let departments: Vec<String> = sqlx::query_scalar(
        "SELECT department
         FROM asset_department_bindings
         WHERE asset_id = $1
         ORDER BY department ASC",
    )
    .bind(asset_id)
    .fetch_all(db)
    .await?;

    let business_services: Vec<String> = sqlx::query_scalar(
        "SELECT business_service
         FROM asset_business_service_bindings
         WHERE asset_id = $1
         ORDER BY business_service ASC",
    )
    .bind(asset_id)
    .fetch_all(db)
    .await?;

    let owners: Vec<AssetOwnerBinding> = sqlx::query_as(
        "SELECT owner_type, owner_ref
         FROM asset_owner_bindings
         WHERE asset_id = $1
         ORDER BY owner_type ASC, owner_ref ASC",
    )
    .bind(asset_id)
    .fetch_all(db)
    .await?;

    let readiness = evaluate_operational_readiness(db, asset_id).await?;

    Ok(AssetBindingsResponse {
        asset_id,
        departments,
        business_services,
        owners,
        readiness,
    })
}

async fn evaluate_operational_readiness(
    db: &sqlx::PgPool,
    asset_id: i64,
) -> AppResult<AssetOperationalReadiness> {
    let department_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM asset_department_bindings
         WHERE asset_id = $1",
    )
    .bind(asset_id)
    .fetch_one(db)
    .await?;

    let business_service_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM asset_business_service_bindings
         WHERE asset_id = $1",
    )
    .bind(asset_id)
    .fetch_one(db)
    .await?;

    let owner_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM asset_owner_bindings
         WHERE asset_id = $1",
    )
    .bind(asset_id)
    .fetch_one(db)
    .await?;

    let mut missing = Vec::new();
    if department_count == 0 {
        missing.push("department_binding".to_string());
    }
    if business_service_count == 0 {
        missing.push("business_service_binding".to_string());
    }
    if owner_count == 0 {
        missing.push("owner_binding".to_string());
    }

    Ok(AssetOperationalReadiness {
        department_count,
        business_service_count,
        owner_count,
        can_transition_operational: missing.is_empty(),
        missing,
    })
}

async fn ensure_asset_exists(db: &sqlx::PgPool, asset_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assets WHERE id = $1)")
        .bind(asset_id)
        .fetch_one(db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!("asset {asset_id} not found")));
    }

    Ok(())
}

async fn get_asset_status(db: &sqlx::PgPool, asset_id: i64) -> AppResult<Option<String>> {
    sqlx::query_scalar("SELECT status FROM assets WHERE id = $1")
        .bind(asset_id)
        .fetch_optional(db)
        .await
        .map_err(AppError::from)
}

fn normalize_string_bindings(
    values: Option<Vec<String>>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<Vec<String>>> {
    let Some(values) = values else {
        return Ok(None);
    };

    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();

    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > max_len {
            return Err(AppError::Validation(format!(
                "{field} item length must be <= {max_len}"
            )));
        }

        let lowered = trimmed.to_ascii_lowercase();
        if seen.insert(lowered) {
            normalized.push(trimmed.to_string());
        }
    }

    if normalized.len() > 200 {
        return Err(AppError::Validation(format!(
            "{field} item count must be <= 200"
        )));
    }

    Ok(Some(normalized))
}

fn normalize_owner_bindings(
    values: Option<Vec<UpsertAssetOwnerBinding>>,
) -> AppResult<Option<Vec<AssetOwnerBinding>>> {
    let Some(values) = values else {
        return Ok(None);
    };

    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();

    for raw in values {
        let owner_type = normalize_owner_type(raw.owner_type)?;
        let owner_ref = required_trimmed(raw.owner_ref, "owner_ref", 128)?;

        let key = format!("{owner_type}:{owner_ref}").to_ascii_lowercase();
        if seen.insert(key) {
            normalized.push(AssetOwnerBinding {
                owner_type,
                owner_ref,
            });
        }
    }

    if normalized.len() > 200 {
        return Err(AppError::Validation(
            "owners item count must be <= 200".to_string(),
        ));
    }

    Ok(Some(normalized))
}

fn normalize_owner_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed(value, "owner_type", 32)?.to_ascii_lowercase();

    match normalized.as_str() {
        OWNER_TYPE_TEAM | OWNER_TYPE_USER | OWNER_TYPE_GROUP | OWNER_TYPE_EXTERNAL => {
            Ok(normalized)
        }
        _ => Err(AppError::Validation(
            "owner_type must be one of: team, user, group, external".to_string(),
        )),
    }
}

fn normalize_lifecycle_status(value: String) -> AppResult<String> {
    let normalized = required_trimmed(value, "status", 32)?.to_ascii_lowercase();

    match normalized.as_str() {
        STATUS_IDLE | STATUS_ONBOARDING | STATUS_OPERATIONAL | STATUS_MAINTENANCE
        | STATUS_RETIRED => Ok(normalized),
        _ => Err(AppError::Validation(
            "status must be one of: idle, onboarding, operational, maintenance, retired"
                .to_string(),
        )),
    }
}

fn required_trimmed(value: String, field: &str, max_len: usize) -> AppResult<String> {
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
