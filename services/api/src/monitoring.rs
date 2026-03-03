use std::time::Duration;

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
    audit::write_from_headers_best_effort,
    error::{AppError, AppResult},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/sources",
            get(list_monitoring_sources).post(create_monitoring_source),
        )
        .route(
            "/sources/{id}/probe",
            axum::routing::post(probe_monitoring_source),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MonitoringSource {
    id: i64,
    name: String,
    source_type: String,
    endpoint: String,
    proxy_endpoint: Option<String>,
    auth_type: String,
    username: Option<String>,
    secret_ref: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: bool,
    last_probe_at: Option<DateTime<Utc>>,
    last_probe_status: Option<String>,
    last_probe_message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct MonitoringSourceProbeResponse {
    source: MonitoringSource,
    reachable: bool,
    status_code: Option<u16>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CreateMonitoringSourceRequest {
    name: String,
    source_type: String,
    endpoint: String,
    proxy_endpoint: Option<String>,
    auth_type: Option<String>,
    username: Option<String>,
    secret_ref: String,
    site: Option<String>,
    department: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListMonitoringSourcesQuery {
    source_type: Option<String>,
    site: Option<String>,
    department: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug)]
struct ProbeResult {
    reachable: bool,
    status_code: Option<u16>,
    message: String,
}

async fn list_monitoring_sources(
    State(state): State<AppState>,
    Query(query): Query<ListMonitoringSourcesQuery>,
) -> AppResult<Json<Vec<MonitoringSource>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at
         FROM monitoring_sources
         WHERE 1=1",
    );

    if let Some(source_type) = trim_optional(query.source_type) {
        builder.push(" AND source_type = ").push_bind(source_type);
    }
    if let Some(site) = trim_optional(query.site) {
        builder.push(" AND site = ").push_bind(site);
    }
    if let Some(department) = trim_optional(query.department) {
        builder.push(" AND department = ").push_bind(department);
    }
    if let Some(is_enabled) = query.is_enabled {
        builder.push(" AND is_enabled = ").push_bind(is_enabled);
    }

    builder.push(" ORDER BY id DESC");
    let items: Vec<MonitoringSource> = builder.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(items))
}

async fn create_monitoring_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateMonitoringSourceRequest>,
) -> AppResult<Json<MonitoringSource>> {
    let name = required_trimmed("name", payload.name, 128)?;
    let source_type = normalize_source_type(payload.source_type)?;
    let endpoint = normalize_endpoint(payload.endpoint)?;
    let proxy_endpoint = normalize_optional_endpoint(payload.proxy_endpoint)?;
    let auth_type = normalize_auth_type(payload.auth_type)?;
    let username = normalize_username(payload.username, &auth_type)?;
    let secret_ref = required_trimmed("secret_ref", payload.secret_ref, 255)?;
    let site = trim_optional(payload.site);
    let department = trim_optional(payload.department);
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let item: MonitoringSource = sqlx::query_as(
        "INSERT INTO monitoring_sources (
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at",
    )
    .bind(name)
    .bind(source_type)
    .bind(endpoint)
    .bind(proxy_endpoint)
    .bind(auth_type)
    .bind(username)
    .bind(secret_ref)
    .bind(site)
    .bind(department)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_create_conflict)?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "monitoring.source.create",
        "monitoring_source",
        Some(item.id.to_string()),
        "success",
        None,
        serde_json::json!({
            "source_type": &item.source_type,
            "endpoint": &item.endpoint,
            "is_enabled": item.is_enabled
        }),
    )
    .await;

    Ok(Json(item))
}

async fn probe_monitoring_source(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<MonitoringSourceProbeResponse>> {
    let source = get_monitoring_source(&state.db, id).await?;
    let probe = probe_endpoint(&source.endpoint).await;
    let status = if probe.reachable {
        "reachable"
    } else {
        "unreachable"
    };

    let updated: MonitoringSource = sqlx::query_as(
        "UPDATE monitoring_sources
         SET
            last_probe_at = NOW(),
            last_probe_status = $2,
            last_probe_message = $3,
            updated_at = NOW()
         WHERE id = $1
         RETURNING
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at",
    )
    .bind(id)
    .bind(status)
    .bind(limit_len(&probe.message, 512))
    .fetch_one(&state.db)
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "monitoring.source.probe",
        "monitoring_source",
        Some(id.to_string()),
        if probe.reachable { "success" } else { "failed" },
        Some(probe.message.clone()),
        serde_json::json!({
            "reachable": probe.reachable,
            "status_code": probe.status_code
        }),
    )
    .await;

    Ok(Json(MonitoringSourceProbeResponse {
        source: updated,
        reachable: probe.reachable,
        status_code: probe.status_code,
        message: probe.message,
    }))
}

async fn get_monitoring_source(db: &sqlx::PgPool, source_id: i64) -> AppResult<MonitoringSource> {
    let item: Option<MonitoringSource> = sqlx::query_as(
        "SELECT
            id,
            name,
            source_type,
            endpoint,
            proxy_endpoint,
            auth_type,
            username,
            secret_ref,
            site,
            department,
            is_enabled,
            last_probe_at,
            last_probe_status,
            last_probe_message,
            created_at,
            updated_at
         FROM monitoring_sources
         WHERE id = $1",
    )
    .bind(source_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("monitoring source {source_id} does not exist")))
}

async fn probe_endpoint(endpoint: &str) -> ProbeResult {
    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(response) => ProbeResult {
            reachable: true,
            status_code: Some(response.status().as_u16()),
            message: format!("probe request succeeded with HTTP {}", response.status()),
        },
        Err(err) => ProbeResult {
            reachable: false,
            status_code: None,
            message: format!("probe request failed: {err}"),
        },
    }
}

fn normalize_source_type(value: String) -> AppResult<String> {
    let normalized = required_trimmed("source_type", value, 32)?.to_ascii_lowercase();
    if normalized != "zabbix" {
        return Err(AppError::Validation(
            "source_type must be 'zabbix'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_auth_type(value: Option<String>) -> AppResult<String> {
    let normalized = value
        .and_then(|raw| trim_optional(Some(raw)))
        .unwrap_or_else(|| "token".to_string())
        .to_ascii_lowercase();

    match normalized.as_str() {
        "token" | "basic" => Ok(normalized),
        _ => Err(AppError::Validation(
            "auth_type must be one of: token, basic".to_string(),
        )),
    }
}

fn normalize_username(value: Option<String>, auth_type: &str) -> AppResult<Option<String>> {
    let username = trim_optional(value);
    if auth_type == "basic" && username.is_none() {
        return Err(AppError::Validation(
            "username is required when auth_type=basic".to_string(),
        ));
    }

    if let Some(ref user) = username {
        if user.len() > 128 {
            return Err(AppError::Validation(
                "username length must be <= 128".to_string(),
            ));
        }
    }
    Ok(username)
}

fn normalize_endpoint(value: String) -> AppResult<String> {
    let endpoint = required_trimmed("endpoint", value, 512)?;
    validate_url(&endpoint, "endpoint")?;
    Ok(endpoint)
}

fn normalize_optional_endpoint(value: Option<String>) -> AppResult<Option<String>> {
    let endpoint = trim_optional(value);
    if let Some(ref endpoint) = endpoint {
        if endpoint.len() > 512 {
            return Err(AppError::Validation(
                "proxy_endpoint length must be <= 512".to_string(),
            ));
        }
        validate_url(endpoint, "proxy_endpoint")?;
    }
    Ok(endpoint)
}

fn validate_url(value: &str, field: &str) -> AppResult<()> {
    let parsed = reqwest::Url::parse(value)
        .map_err(|_| AppError::Validation(format!("{field} must be a valid URL")))?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::Validation(format!(
            "{field} must use http or https scheme"
        )));
    }

    Ok(())
}

fn required_trimmed(field: &str, value: String, max_len: usize) -> AppResult<String> {
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

fn limit_len(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    value.chars().take(max_len).collect()
}

fn map_create_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("monitoring source name already exists".to_string());
        }
    }
    AppError::Database(err)
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_auth_type, normalize_optional_endpoint, normalize_source_type, normalize_username,
    };

    #[test]
    fn validates_source_type() {
        assert!(normalize_source_type("zabbix".to_string()).is_ok());
        assert!(normalize_source_type("snmp".to_string()).is_err());
    }

    #[test]
    fn validates_auth_type() {
        assert!(normalize_auth_type(None).is_ok());
        assert!(normalize_auth_type(Some("basic".to_string())).is_ok());
        assert!(normalize_auth_type(Some("oauth".to_string())).is_err());
    }

    #[test]
    fn basic_auth_requires_username() {
        assert!(normalize_username(None, "basic").is_err());
        assert!(normalize_username(Some("ops".to_string()), "basic").is_ok());
    }

    #[test]
    fn validates_optional_proxy_url() {
        assert!(normalize_optional_endpoint(Some("http://127.0.0.1:8080".to_string())).is_ok());
        assert!(normalize_optional_endpoint(Some("ftp://host".to_string())).is_err());
    }
}
