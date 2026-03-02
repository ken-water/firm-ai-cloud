use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Postgres;
use sqlx::QueryBuilder;

use crate::state::AppState;
use crate::{
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/discovery/notification-channels",
            get(list_notification_channels).post(create_notification_channel),
        )
        .route(
            "/discovery/notification-templates",
            get(list_notification_templates).post(create_notification_template),
        )
        .route(
            "/discovery/notification-subscriptions",
            get(list_notification_subscriptions).post(create_notification_subscription),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NotificationChannel {
    id: i64,
    name: String,
    channel_type: String,
    target: String,
    config: Value,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NotificationTemplate {
    id: i64,
    event_type: String,
    title_template: String,
    body_template: String,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct NotificationSubscription {
    id: i64,
    channel_id: i64,
    event_type: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateNotificationChannelRequest {
    name: String,
    channel_type: String,
    target: String,
    config: Option<Value>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateNotificationTemplateRequest {
    event_type: String,
    title_template: String,
    body_template: String,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateNotificationSubscriptionRequest {
    channel_id: i64,
    event_type: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListNotificationChannelsQuery {
    channel_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListNotificationTemplatesQuery {
    event_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListNotificationSubscriptionsQuery {
    event_type: Option<String>,
    channel_id: Option<i64>,
}

async fn create_notification_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNotificationChannelRequest>,
) -> AppResult<Json<NotificationChannel>> {
    let name = required_trimmed("name", payload.name)?;
    let channel_type = normalize_channel_type(payload.channel_type)?;
    let target = required_trimmed("target", payload.target)?;
    let config = normalize_config(payload.config)?;
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let item: NotificationChannel = sqlx::query_as(
        "INSERT INTO discovery_notification_channels (name, channel_type, target, config, is_enabled)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, channel_type, target, config, is_enabled, created_at, updated_at",
    )
    .bind(name)
    .bind(channel_type)
    .bind(target)
    .bind(config)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.notification_channel.create",
        "notification_channel",
        Some(item.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "channel_type": &item.channel_type,
            "name": &item.name
        }),
    )
    .await;

    Ok(Json(item))
}

async fn list_notification_channels(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationChannelsQuery>,
) -> AppResult<Json<Vec<NotificationChannel>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, name, channel_type, target, config, is_enabled, created_at, updated_at
         FROM discovery_notification_channels
         WHERE 1=1",
    );

    if let Some(channel_type) = trim_optional(query.channel_type) {
        builder.push(" AND channel_type = ").push_bind(channel_type);
    }

    builder.push(" ORDER BY id DESC");

    let items: Vec<NotificationChannel> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn create_notification_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNotificationTemplateRequest>,
) -> AppResult<Json<NotificationTemplate>> {
    let event_type = normalize_event_type(payload.event_type)?;
    let title_template = required_trimmed("title_template", payload.title_template)?;
    let body_template = required_trimmed("body_template", payload.body_template)?;
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let item: NotificationTemplate = sqlx::query_as(
        "INSERT INTO discovery_notification_templates (event_type, title_template, body_template, is_enabled)
         VALUES ($1, $2, $3, $4)
         RETURNING id, event_type, title_template, body_template, is_enabled, created_at, updated_at",
    )
    .bind(event_type)
    .bind(title_template)
    .bind(body_template)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.notification_template.create",
        "notification_template",
        Some(item.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "event_type": &item.event_type
        }),
    )
    .await;

    Ok(Json(item))
}

async fn list_notification_templates(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationTemplatesQuery>,
) -> AppResult<Json<Vec<NotificationTemplate>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, event_type, title_template, body_template, is_enabled, created_at, updated_at
         FROM discovery_notification_templates
         WHERE 1=1",
    );

    if let Some(event_type) = trim_optional(query.event_type) {
        builder.push(" AND event_type = ").push_bind(event_type);
    }

    builder.push(" ORDER BY id DESC");

    let items: Vec<NotificationTemplate> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn create_notification_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNotificationSubscriptionRequest>,
) -> AppResult<Json<NotificationSubscription>> {
    let event_type = normalize_event_type(payload.event_type)?;
    let site = trim_optional(payload.site);
    let department = trim_optional(payload.department);
    let is_enabled = payload.is_enabled.unwrap_or(true);

    ensure_channel_exists(&state.db, payload.channel_id).await?;

    let item: NotificationSubscription = sqlx::query_as(
        "INSERT INTO discovery_notification_subscriptions (channel_id, event_type, site, department, is_enabled)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, channel_id, event_type, site, department, is_enabled, created_at, updated_at",
    )
    .bind(payload.channel_id)
    .bind(event_type)
    .bind(site)
    .bind(department)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_subscription_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "cmdb.notification_subscription.create",
        "notification_subscription",
        Some(item.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "channel_id": item.channel_id,
            "event_type": &item.event_type
        }),
    )
    .await;

    Ok(Json(item))
}

async fn list_notification_subscriptions(
    State(state): State<AppState>,
    Query(query): Query<ListNotificationSubscriptionsQuery>,
) -> AppResult<Json<Vec<NotificationSubscription>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, channel_id, event_type, site, department, is_enabled, created_at, updated_at
         FROM discovery_notification_subscriptions
         WHERE 1=1",
    );

    if let Some(event_type) = trim_optional(query.event_type) {
        builder.push(" AND event_type = ").push_bind(event_type);
    }
    if let Some(channel_id) = query.channel_id {
        builder.push(" AND channel_id = ").push_bind(channel_id);
    }

    builder.push(" ORDER BY id DESC");

    let items: Vec<NotificationSubscription> =
        builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn ensure_channel_exists(db: &sqlx::PgPool, channel_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM discovery_notification_channels WHERE id = $1)",
    )
    .bind(channel_id)
    .fetch_one(db)
    .await?;

    if !exists {
        return Err(AppError::Validation(format!(
            "notification channel {channel_id} does not exist"
        )));
    }

    Ok(())
}

fn normalize_channel_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed("channel_type", value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "email" | "webhook" => Ok(normalized),
        _ => Err(AppError::Validation(
            "channel_type must be one of: email, webhook".to_string(),
        )),
    }
}

fn normalize_event_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed("event_type", value)?.to_ascii_lowercase();
    if normalized.len() > 64 {
        return Err(AppError::Validation(
            "event_type length must be <= 64".to_string(),
        ));
    }
    if !normalized.chars().all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_' || ch == '-'
    }) {
        return Err(AppError::Validation(
            "event_type contains invalid characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_config(config: Option<Value>) -> AppResult<Value> {
    let value = config.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !value.is_object() {
        return Err(AppError::Validation(
            "config must be a JSON object".to_string(),
        ));
    }
    Ok(value)
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

fn map_subscription_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("notification subscription already exists".to_string());
        }
    }
    AppError::Database(err)
}
