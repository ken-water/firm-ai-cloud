use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder};
use tracing::warn;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

const AUTH_USER_HEADER: &str = "x-auth-user";
const AUDIT_WRITE_SLOW_THRESHOLD: Duration = Duration::from_millis(500);

pub fn routes() -> Router<AppState> {
    Router::new().route("/logs", get(list_audit_logs))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AuditLogRecord {
    id: i64,
    actor: String,
    action: String,
    target_type: String,
    target_id: Option<String>,
    result: String,
    message: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListAuditLogsResponse {
    items: Vec<AuditLogRecord>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Deserialize, Default)]
struct ListAuditLogsQuery {
    actor: Option<String>,
    action: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    result: Option<String>,
    time_from: Option<String>,
    time_to: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AuditLogWriteInput {
    pub actor: String,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub result: String,
    pub message: Option<String>,
    pub metadata: Value,
}

pub async fn write_audit_log(db: &sqlx::PgPool, mut input: AuditLogWriteInput) -> AppResult<()> {
    let started_at = Instant::now();
    input.actor = normalize_field("actor", input.actor, 128)?;
    input.action = normalize_field("action", input.action, 128)?;
    input.target_type = normalize_field("target_type", input.target_type, 64)?;
    input.result = normalize_field("result", input.result, 32)?;
    let action_for_log = input.action.clone();

    let target_id = input
        .target_id
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .map(|value| {
            if value.len() > 128 {
                value[..128].to_string()
            } else {
                value
            }
        });

    let metadata = normalize_metadata(input.metadata)?;
    let message = input.message.map(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "".to_string()
        } else {
            trimmed.to_string()
        }
    });

    sqlx::query(
        "INSERT INTO audit_logs (actor, action, target_type, target_id, result, message, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(input.actor)
    .bind(input.action)
    .bind(input.target_type)
    .bind(target_id)
    .bind(input.result)
    .bind(message.filter(|text| !text.is_empty()))
    .bind(metadata)
    .execute(db)
    .await?;

    let elapsed = started_at.elapsed();
    if elapsed > AUDIT_WRITE_SLOW_THRESHOLD {
        warn!(
            action = %action_for_log,
            elapsed_ms = elapsed.as_millis(),
            "slow audit log write detected"
        );
    }

    Ok(())
}

pub async fn write_audit_log_best_effort(db: &sqlx::PgPool, input: AuditLogWriteInput) {
    let db = db.clone();
    tokio::spawn(async move {
        if let Err(err) = write_audit_log(&db, input).await {
            warn!(error = ?err, "failed to write audit log");
        }
    });
}

pub async fn write_from_headers_best_effort(
    db: &sqlx::PgPool,
    headers: &HeaderMap,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    result: &str,
    message: Option<String>,
    metadata: Value,
) {
    let actor = actor_from_headers(headers).unwrap_or_else(|| "unknown".to_string());
    write_audit_log_best_effort(
        db,
        AuditLogWriteInput {
            actor,
            action: action.to_string(),
            target_type: target_type.to_string(),
            target_id,
            result: result.to_string(),
            message,
            metadata,
        },
    )
    .await;
}

pub fn actor_from_headers(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(AUTH_USER_HEADER)?;
    let value = raw.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

async fn list_audit_logs(
    State(state): State<AppState>,
    Query(query): Query<ListAuditLogsQuery>,
) -> AppResult<Json<ListAuditLogsResponse>> {
    let limit = query.limit.unwrap_or(50).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    let actor = trim_optional(query.actor);
    let action = trim_optional(query.action);
    let target_type = trim_optional(query.target_type);
    let target_id = trim_optional(query.target_id);
    let result = trim_optional(query.result);
    let time_from = parse_time_filter(query.time_from, "time_from")?;
    let time_to = parse_time_filter(query.time_to, "time_to")?;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM audit_logs WHERE 1=1");
    append_audit_filters(
        &mut count_builder,
        actor.clone(),
        action.clone(),
        target_type.clone(),
        target_id.clone(),
        result.clone(),
        time_from,
        time_to,
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, actor, action, target_type, target_id, result, message, metadata, created_at
         FROM audit_logs
         WHERE 1=1",
    );
    append_audit_filters(
        &mut list_builder,
        actor,
        action,
        target_type,
        target_id,
        result,
        time_from,
        time_to,
    );
    list_builder
        .push(" ORDER BY created_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<AuditLogRecord> = list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListAuditLogsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

fn append_audit_filters(
    builder: &mut QueryBuilder<Postgres>,
    actor: Option<String>,
    action: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
    result: Option<String>,
    time_from: Option<DateTime<Utc>>,
    time_to: Option<DateTime<Utc>>,
) {
    if let Some(actor) = actor {
        builder
            .push(" AND actor ILIKE ")
            .push_bind(format!("%{actor}%"));
    }
    if let Some(action) = action {
        builder
            .push(" AND action ILIKE ")
            .push_bind(format!("%{action}%"));
    }
    if let Some(target_type) = target_type {
        builder
            .push(" AND target_type ILIKE ")
            .push_bind(format!("%{target_type}%"));
    }
    if let Some(target_id) = target_id {
        builder
            .push(" AND target_id ILIKE ")
            .push_bind(format!("%{target_id}%"));
    }
    if let Some(result) = result {
        builder
            .push(" AND result ILIKE ")
            .push_bind(format!("%{result}%"));
    }
    if let Some(time_from) = time_from {
        builder.push(" AND created_at >= ").push_bind(time_from);
    }
    if let Some(time_to) = time_to {
        builder.push(" AND created_at <= ").push_bind(time_to);
    }
}

fn parse_time_filter(value: Option<String>, field: &str) -> AppResult<Option<DateTime<Utc>>> {
    let Some(raw) = trim_optional(value) else {
        return Ok(None);
    };

    let parsed = DateTime::parse_from_rfc3339(&raw)
        .map_err(|_| AppError::Validation(format!("{field} must be RFC3339 datetime")))?;

    Ok(Some(parsed.with_timezone(&Utc)))
}

fn normalize_field(field: &str, value: String, max_len: usize) -> AppResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if normalized.len() > max_len {
        return Err(AppError::Validation(format!(
            "{field} length must be <= {max_len}"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_metadata(value: Value) -> AppResult<Value> {
    if value.is_object() {
        return Ok(value);
    }
    if value.is_null() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    Err(AppError::Validation(
        "audit metadata must be a JSON object".to_string(),
    ))
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
