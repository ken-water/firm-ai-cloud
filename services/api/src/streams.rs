use std::{convert::Infallible, time::Duration};

use async_stream::stream;
use axum::{
    Router,
    extract::{Query, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    audit::write_from_headers_best_effort,
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const STREAM_HEARTBEAT_SECONDS: u64 = 5;
const STREAM_STALE_AFTER_SECONDS: i64 = 15;
const STREAM_RECONNECT_AFTER_MS: u64 = 3_000;
const AUTH_SITE_HEADER: &str = "x-auth-site";
const AUTH_DEPARTMENT_HEADER: &str = "x-auth-department";

pub fn routes() -> Router<AppState> {
    Router::new().route("/sse", get(stream_sse))
}

#[derive(Debug, Deserialize, Default)]
struct StreamSseQuery {
    site: Option<String>,
    department: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct StreamScope {
    site: Option<String>,
    department: Option<String>,
    severity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamSeverityFilter {
    All,
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Serialize)]
struct StreamEventEnvelope {
    event_type: String,
    scope: StreamScope,
    timestamp: DateTime<Utc>,
    payload: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct StreamAlertRow {
    id: i64,
    asset_id: i64,
    asset_name: String,
    asset_class: String,
    asset_site: Option<String>,
    asset_department: Option<String>,
    job_status: String,
    attempt: i32,
    max_attempts: i32,
    requested_at: DateTime<Utc>,
    last_error: Option<String>,
}

async fn stream_sse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StreamSseQuery>,
) -> AppResult<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>> {
    let user = resolve_auth_user(&state, &headers).await?;
    let roles = load_user_roles(&state.db, &user).await?;
    let mut scope = normalize_scope(query)?;
    enforce_scope_access(&roles, &headers, &mut scope)?;

    let connection_id = Uuid::new_v4().to_string();
    write_from_headers_best_effort(
        &state.db,
        &headers,
        "streams.sse.subscribe",
        "stream",
        Some(connection_id.clone()),
        "success",
        None,
        json!({
            "scope": {
                "site": scope.site,
                "department": scope.department,
                "severity": scope.severity
            },
            "heartbeat_seconds": STREAM_HEARTBEAT_SECONDS,
            "stale_after_seconds": STREAM_STALE_AFTER_SECONDS,
            "reconnect_after_ms": STREAM_RECONNECT_AFTER_MS
        }),
    )
    .await;

    let db = state.db.clone();
    let scope_for_stream = scope.clone();
    let mut last_emitted_job_id = fetch_latest_sync_job_id(&db, &scope_for_stream).await?;

    let stream = stream! {
        let mut event_id: u64 = 1;
        let connected = StreamEventEnvelope {
            event_type: "stream.connected".to_string(),
            scope: scope_for_stream.clone(),
            timestamp: Utc::now(),
            payload: json!({
                "connection_id": connection_id,
                "heartbeat_interval_seconds": STREAM_HEARTBEAT_SECONDS,
                "stale_after_seconds": STREAM_STALE_AFTER_SECONDS,
                "reconnect_after_ms": STREAM_RECONNECT_AFTER_MS
            }),
        };
        yield encode_sse_event(event_id, connected);
        event_id += 1;

        let test_event = StreamEventEnvelope {
            event_type: "alert.test".to_string(),
            scope: scope_for_stream.clone(),
            timestamp: Utc::now(),
            payload: json!({
                "severity": scope_for_stream.severity,
                "message": "SSE baseline test event: subscription is active",
                "source": "stream_baseline"
            }),
        };
        yield encode_sse_event(event_id, test_event);
        event_id += 1;

        let mut interval = tokio::time::interval(Duration::from_secs(STREAM_HEARTBEAT_SECONDS));
        let mut last_alert_at = Utc::now();
        let mut stale_emitted = false;

        loop {
            interval.tick().await;
            let now = Utc::now();
            match fetch_recent_stream_alerts(&db, &scope_for_stream, last_emitted_job_id, 20).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        if !stale_emitted
                            && (now - last_alert_at).num_seconds() >= STREAM_STALE_AFTER_SECONDS
                        {
                            let stale_event = StreamEventEnvelope {
                                event_type: "stream.stale".to_string(),
                                scope: scope_for_stream.clone(),
                                timestamp: now,
                                payload: json!({
                                    "reason": "no new monitoring sync events in stale window",
                                    "stale_after_seconds": STREAM_STALE_AFTER_SECONDS,
                                    "guidance": "mark stream as delayed and keep reconnecting"
                                }),
                            };
                            yield encode_sse_event(event_id, stale_event);
                            event_id += 1;
                            stale_emitted = true;
                        }
                    } else {
                        if stale_emitted {
                            let recovered = StreamEventEnvelope {
                                event_type: "stream.recovered".to_string(),
                                scope: scope_for_stream.clone(),
                                timestamp: now,
                                payload: json!({
                                    "message": "new events resumed",
                                }),
                            };
                            yield encode_sse_event(event_id, recovered);
                            event_id += 1;
                            stale_emitted = false;
                        }

                        for row in rows {
                            if row.id > last_emitted_job_id {
                                last_emitted_job_id = row.id;
                            }
                            let severity = map_job_status_to_severity(&row.job_status);
                            let alert_event = StreamEventEnvelope {
                                event_type: "alert.monitoring_sync".to_string(),
                                scope: StreamScope {
                                    site: row.asset_site.clone(),
                                    department: row.asset_department.clone(),
                                    severity: severity.to_string(),
                                },
                                timestamp: row.requested_at,
                                payload: json!({
                                    "job_id": row.id,
                                    "asset": {
                                        "id": row.asset_id,
                                        "name": row.asset_name,
                                        "class": row.asset_class,
                                        "site": row.asset_site,
                                        "department": row.asset_department
                                    },
                                    "status": row.job_status,
                                    "severity": severity,
                                    "attempt": row.attempt,
                                    "max_attempts": row.max_attempts,
                                    "last_error": row.last_error
                                }),
                            };
                            yield encode_sse_event(event_id, alert_event);
                            event_id += 1;
                        }
                        last_alert_at = now;
                    }
                }
                Err(err) => {
                    let error_event = StreamEventEnvelope {
                        event_type: "stream.error".to_string(),
                        scope: scope_for_stream.clone(),
                        timestamp: now,
                        payload: json!({
                            "message": format!("stream polling failed: {err}")
                        }),
                    };
                    yield encode_sse_event(event_id, error_event);
                    event_id += 1;
                }
            }

            let heartbeat = StreamEventEnvelope {
                event_type: "stream.heartbeat".to_string(),
                scope: scope_for_stream.clone(),
                timestamp: now,
                payload: json!({
                    "connection_alive": true,
                    "heartbeat_interval_seconds": STREAM_HEARTBEAT_SECONDS,
                }),
            };
            yield encode_sse_event(event_id, heartbeat);
            event_id += 1;
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keepalive"),
    ))
}

fn encode_sse_event(event_id: u64, envelope: StreamEventEnvelope) -> Result<Event, Infallible> {
    let event_name = envelope.event_type.clone();
    let body = match serde_json::to_string(&envelope) {
        Ok(value) => value,
        Err(err) => json!({
            "event_type": "stream.error",
            "timestamp": Utc::now(),
            "payload": {
                "message": format!("failed to serialize stream event: {err}")
            }
        })
        .to_string(),
    };

    Ok(Event::default()
        .id(event_id.to_string())
        .event(event_name)
        .data(body))
}

fn normalize_scope(query: StreamSseQuery) -> AppResult<StreamScope> {
    Ok(StreamScope {
        site: normalize_scope_value(query.site, "site", 128)?,
        department: normalize_scope_value(query.department, "department", 128)?,
        severity: normalize_severity_filter(query.severity)?
            .as_str()
            .to_string(),
    })
}

fn normalize_scope_value(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let normalized = trim_optional(value);
    if let Some(ref value) = normalized {
        if value.len() > max_len {
            return Err(AppError::Validation(format!(
                "{field} length must be <= {max_len}"
            )));
        }
    }
    Ok(normalized)
}

fn normalize_severity_filter(value: Option<String>) -> AppResult<StreamSeverityFilter> {
    let normalized = value
        .and_then(|raw| trim_optional(Some(raw)))
        .unwrap_or_else(|| "all".to_string())
        .to_ascii_lowercase();

    match normalized.as_str() {
        "all" => Ok(StreamSeverityFilter::All),
        "critical" => Ok(StreamSeverityFilter::Critical),
        "warning" => Ok(StreamSeverityFilter::Warning),
        "info" => Ok(StreamSeverityFilter::Info),
        _ => Err(AppError::Validation(
            "severity must be one of: all, critical, warning, info".to_string(),
        )),
    }
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

fn read_scope_header(headers: &HeaderMap, key: &str, max_len: usize) -> AppResult<Option<String>> {
    let Some(raw) = headers.get(key) else {
        return Ok(None);
    };

    let value = raw
        .to_str()
        .map_err(|_| AppError::Forbidden(format!("{key} header is invalid")))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Forbidden(format!("{key} header cannot be empty")));
    }
    if trimmed.len() > max_len {
        return Err(AppError::Forbidden(format!(
            "{key} header length must be <= {max_len}"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn enforce_scope_access(
    roles: &[String],
    headers: &HeaderMap,
    scope: &mut StreamScope,
) -> AppResult<()> {
    if roles.iter().any(|role| role == "admin") {
        return Ok(());
    }

    if scope.site.is_none() && scope.department.is_none() {
        return Err(AppError::Forbidden(
            "scope denied: non-admin stream subscriptions must include site or department filter"
                .to_string(),
        ));
    }

    let allowed_site = read_scope_header(headers, AUTH_SITE_HEADER, 128)?;
    if let Some(allowed_site) = allowed_site {
        match scope.site.clone() {
            Some(requested) if !requested.eq_ignore_ascii_case(&allowed_site) => {
                return Err(AppError::Forbidden(format!(
                    "scope denied: requested site '{}' is outside authorized site '{}'",
                    requested, allowed_site
                )));
            }
            None => {
                scope.site = Some(allowed_site);
            }
            _ => {}
        }
    }

    let allowed_department = read_scope_header(headers, AUTH_DEPARTMENT_HEADER, 128)?;
    if let Some(allowed_department) = allowed_department {
        match scope.department.clone() {
            Some(requested) if !requested.eq_ignore_ascii_case(&allowed_department) => {
                return Err(AppError::Forbidden(format!(
                    "scope denied: requested department '{}' is outside authorized department '{}'",
                    requested, allowed_department
                )));
            }
            None => {
                scope.department = Some(allowed_department);
            }
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

async fn fetch_latest_sync_job_id(db: &sqlx::PgPool, scope: &StreamScope) -> AppResult<i64> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COALESCE(MAX(j.id), 0)
         FROM cmdb_monitoring_sync_jobs j
         INNER JOIN assets a ON a.id = j.asset_id
         WHERE 1=1",
    );
    append_scope_filters(&mut qb, scope.site.as_deref(), scope.department.as_deref());
    append_severity_filter(&mut qb, scope.severity.as_str());

    qb.build_query_scalar()
        .fetch_one(db)
        .await
        .map_err(AppError::from)
}

async fn fetch_recent_stream_alerts(
    db: &sqlx::PgPool,
    scope: &StreamScope,
    last_seen_id: i64,
    limit: i64,
) -> AppResult<Vec<StreamAlertRow>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            j.id,
            j.asset_id,
            a.name AS asset_name,
            a.asset_class,
            a.site AS asset_site,
            a.department AS asset_department,
            j.status AS job_status,
            j.attempt,
            j.max_attempts,
            j.requested_at,
            j.last_error
         FROM cmdb_monitoring_sync_jobs j
         INNER JOIN assets a ON a.id = j.asset_id
         WHERE j.id > ",
    );
    qb.push_bind(last_seen_id);
    append_scope_filters(&mut qb, scope.site.as_deref(), scope.department.as_deref());
    append_severity_filter(&mut qb, scope.severity.as_str());
    qb.push(" ORDER BY j.id ASC LIMIT ").push_bind(limit);

    qb.build_query_as()
        .fetch_all(db)
        .await
        .map_err(AppError::from)
}

fn append_scope_filters<'a>(
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

fn append_severity_filter(builder: &mut QueryBuilder<Postgres>, severity: &str) {
    match severity {
        "critical" => {
            builder.push(" AND j.status IN ('failed', 'dead_letter')");
        }
        "warning" => {
            builder.push(" AND j.status IN ('pending', 'running')");
        }
        "info" => {
            builder.push(" AND j.status IN ('success', 'skipped')");
        }
        _ => {}
    }
}

fn map_job_status_to_severity(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "failed" | "dead_letter" => "critical",
        "pending" | "running" => "warning",
        "success" | "skipped" => "info",
        _ => "info",
    }
}

impl StreamSeverityFilter {
    fn as_str(self) -> &'static str {
        match self {
            StreamSeverityFilter::All => "all",
            StreamSeverityFilter::Critical => "critical",
            StreamSeverityFilter::Warning => "warning",
            StreamSeverityFilter::Info => "info",
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::{
        AUTH_SITE_HEADER, StreamScope, enforce_scope_access, normalize_scope_value,
        normalize_severity_filter,
    };

    #[test]
    fn validates_severity_filter() {
        assert!(normalize_severity_filter(None).is_ok());
        assert!(normalize_severity_filter(Some("all".to_string())).is_ok());
        assert!(normalize_severity_filter(Some("critical".to_string())).is_ok());
        assert!(normalize_severity_filter(Some("warning".to_string())).is_ok());
        assert!(normalize_severity_filter(Some("info".to_string())).is_ok());
        assert!(normalize_severity_filter(Some("unknown".to_string())).is_err());
    }

    #[test]
    fn validates_scope_value_length() {
        assert!(normalize_scope_value(Some("dc-a".to_string()), "site", 128).is_ok());
        assert!(normalize_scope_value(Some(" ".to_string()), "site", 128).is_ok());
    }

    #[test]
    fn non_admin_scope_requires_site_or_department() {
        let mut scope = StreamScope {
            site: None,
            department: None,
            severity: "all".to_string(),
        };
        let headers = HeaderMap::new();
        let roles = vec!["viewer".to_string()];
        assert!(enforce_scope_access(&roles, &headers, &mut scope).is_err());
    }

    #[test]
    fn denies_mismatched_header_scope() {
        let mut scope = StreamScope {
            site: Some("dc-b".to_string()),
            department: Some("ops".to_string()),
            severity: "warning".to_string(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_SITE_HEADER, HeaderValue::from_static("dc-a"));
        let roles = vec!["operator".to_string()];
        let err = enforce_scope_access(&roles, &headers, &mut scope).expect_err("should deny");
        assert!(
            err.to_string().contains("outside authorized site"),
            "unexpected message: {}",
            err
        );
    }

    #[test]
    fn admin_can_subscribe_without_scope() {
        let mut scope = StreamScope {
            site: None,
            department: None,
            severity: "all".to_string(),
        };
        let headers = HeaderMap::new();
        let roles = vec!["admin".to_string()];
        assert!(enforce_scope_access(&roles, &headers, &mut scope).is_ok());
    }
}
