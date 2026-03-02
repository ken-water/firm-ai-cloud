use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

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
) -> AppResult<Json<DiscoveryJob>> {
    let job: Option<DiscoveryJob> = sqlx::query_as(
        "UPDATE discovery_jobs
         SET status = 'queued',
             last_run_at = NOW(),
             last_run_status = 'queued',
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1
         RETURNING id, name, source_type, scope, schedule, status, is_enabled, last_run_at, last_run_status, last_error, created_at, updated_at",
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?;

    let job = job.ok_or_else(|| AppError::NotFound(format!("discovery job {job_id} not found")))?;
    Ok(Json(job))
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
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "source_type can only contain lowercase letters, numbers, '-', '_'".to_string(),
        ));
    }
    if normalized.len() > 32 {
        return Err(AppError::Validation(
            "source_type length must be <= 32".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_scope(scope: Option<Value>) -> AppResult<Value> {
    let scope = scope.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !scope.is_object() {
        return Err(AppError::Validation(
            "scope must be a JSON object".to_string(),
        ));
    }
    Ok(scope)
}
