use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const DEFAULT_LIMIT: u32 = 40;
const MAX_LIMIT: u32 = 200;
const MAX_OWNER_LEN: usize = 128;
const MAX_TEXT_LEN: usize = 1024;

const INCIDENT_STATUS_TRIAGE: &str = "triage";
const INCIDENT_STATUS_IN_PROGRESS: &str = "in_progress";
const INCIDENT_STATUS_BLOCKED: &str = "blocked";
const INCIDENT_STATUS_MITIGATED: &str = "mitigated";
const INCIDENT_STATUS_POSTMORTEM: &str = "postmortem";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/incidents", get(list_incident_commands))
        .route("/cockpit/incidents/{alert_id}", get(get_incident_command))
        .route(
            "/cockpit/incidents/{alert_id}/command",
            post(upsert_incident_command),
        )
}

#[derive(Debug, Deserialize, Default)]
struct ListIncidentCommandsQuery {
    status: Option<String>,
    site: Option<String>,
    department: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct UpsertIncidentCommandRequest {
    status: Option<String>,
    owner: Option<String>,
    eta_at: Option<String>,
    blocker: Option<String>,
    summary: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct IncidentCommandListItem {
    alert_id: i64,
    alert_source: String,
    alert_key: String,
    title: String,
    severity: String,
    alert_status: String,
    site: Option<String>,
    department: Option<String>,
    command_status: String,
    command_owner: String,
    eta_at: Option<DateTime<Utc>>,
    blocker: Option<String>,
    summary: Option<String>,
    updated_by: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
struct IncidentCommandEvent {
    id: i64,
    alert_id: i64,
    event_type: String,
    from_status: Option<String>,
    to_status: String,
    command_owner: String,
    eta_at: Option<DateTime<Utc>>,
    blocker: Option<String>,
    summary: Option<String>,
    note: Option<String>,
    actor: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListIncidentCommandsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<IncidentCommandListItem>,
}

#[derive(Debug, Serialize)]
struct IncidentCommandDetailResponse {
    generated_at: DateTime<Utc>,
    item: IncidentCommandListItem,
    timeline: Vec<IncidentCommandEvent>,
}

#[derive(Debug, Clone, FromRow)]
struct IncidentCommandRow {
    command_status: String,
    command_owner: String,
    eta_at: Option<DateTime<Utc>>,
    blocker: Option<String>,
    summary: Option<String>,
}

async fn list_incident_commands(
    State(state): State<AppState>,
    Query(query): Query<ListIncidentCommandsQuery>,
) -> AppResult<Json<ListIncidentCommandsResponse>> {
    let status = query.status.map(normalize_incident_status).transpose()?;
    let site = trim_optional(query.site, 128);
    let department = trim_optional(query.department, 128);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);

    let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(*)
         FROM unified_alerts ua
         LEFT JOIN ops_incident_commands c ON c.alert_id = ua.id
         WHERE ua.status IN ('open', 'acknowledged')",
    );
    append_incident_filters(
        &mut count_builder,
        status.clone(),
        site.clone(),
        department.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT ua.id AS alert_id,
                ua.alert_source,
                ua.alert_key,
                ua.title,
                ua.severity,
                ua.status AS alert_status,
                ua.site,
                ua.department,
                COALESCE(c.command_status, 'triage') AS command_status,
                COALESCE(c.command_owner, 'unassigned') AS command_owner,
                c.eta_at,
                c.blocker,
                c.summary,
                COALESCE(c.updated_by, 'system') AS updated_by,
                COALESCE(c.updated_at, ua.last_seen_at) AS updated_at
         FROM unified_alerts ua
         LEFT JOIN ops_incident_commands c ON c.alert_id = ua.id
         WHERE ua.status IN ('open', 'acknowledged')",
    );
    append_incident_filters(&mut list_builder, status, site, department);
    list_builder
        .push(
            " ORDER BY
                CASE ua.severity
                  WHEN 'critical' THEN 3
                  WHEN 'warning' THEN 2
                  ELSE 1
                END DESC,
                COALESCE(c.updated_at, ua.last_seen_at) DESC,
                ua.id DESC",
        )
        .push(" LIMIT ")
        .push_bind(limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let items: Vec<IncidentCommandListItem> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListIncidentCommandsResponse {
        generated_at: Utc::now(),
        total,
        limit,
        offset,
        items,
    }))
}

fn append_incident_filters(
    builder: &mut QueryBuilder<Postgres>,
    status: Option<String>,
    site: Option<String>,
    department: Option<String>,
) {
    if let Some(status) = status {
        builder
            .push(" AND COALESCE(c.command_status, 'triage') = ")
            .push_bind(status);
    }
    if let Some(site) = site {
        builder.push(" AND ua.site = ").push_bind(site);
    }
    if let Some(department) = department {
        builder.push(" AND ua.department = ").push_bind(department);
    }
}

async fn get_incident_command(
    State(state): State<AppState>,
    Path(alert_id): Path<i64>,
) -> AppResult<Json<IncidentCommandDetailResponse>> {
    let item = load_incident_item(&state.db, alert_id).await?;
    let timeline = load_incident_timeline(&state.db, alert_id).await?;

    Ok(Json(IncidentCommandDetailResponse {
        generated_at: Utc::now(),
        item,
        timeline,
    }))
}

async fn upsert_incident_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(alert_id): Path<i64>,
    Json(payload): Json<UpsertIncidentCommandRequest>,
) -> AppResult<Json<IncidentCommandDetailResponse>> {
    if alert_id <= 0 {
        return Err(AppError::Validation(
            "alert id must be a positive integer".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    ensure_alert_exists(&state.db, alert_id).await?;

    let mut tx = state.db.begin().await?;
    let current: Option<IncidentCommandRow> = sqlx::query_as(
        "SELECT command_status, command_owner, eta_at, blocker, summary
         FROM ops_incident_commands
         WHERE alert_id = $1",
    )
    .bind(alert_id)
    .fetch_optional(&mut *tx)
    .await?;

    let current_status = current
        .as_ref()
        .map(|item| item.command_status.as_str())
        .unwrap_or(INCIDENT_STATUS_TRIAGE);
    let next_status = payload
        .status
        .map(normalize_incident_status)
        .transpose()?
        .unwrap_or_else(|| current_status.to_string());

    let next_owner = payload
        .owner
        .map(|value| normalize_required_text("owner", value, MAX_OWNER_LEN))
        .transpose()?
        .unwrap_or_else(|| {
            current
                .as_ref()
                .map(|item| item.command_owner.clone())
                .unwrap_or_else(|| actor.clone())
        });
    let next_eta_at = if payload.eta_at.is_some() {
        parse_optional_eta(payload.eta_at)?
    } else {
        current.as_ref().and_then(|item| item.eta_at)
    };
    let next_blocker = if payload.blocker.is_some() {
        normalize_optional_text(payload.blocker, "blocker", MAX_TEXT_LEN)?
    } else {
        current.as_ref().and_then(|item| item.blocker.clone())
    };
    let next_summary = if payload.summary.is_some() {
        normalize_optional_text(payload.summary, "summary", MAX_TEXT_LEN)?
    } else {
        current.as_ref().and_then(|item| item.summary.clone())
    };
    let note = normalize_optional_text(payload.note, "note", MAX_TEXT_LEN)?;

    validate_incident_transition(current_status, next_status.as_str())?;
    validate_incident_required_fields(next_status.as_str(), next_owner.as_str(), next_eta_at)?;

    let changed = current
        .as_ref()
        .map(|item| {
            item.command_status != next_status
                || item.command_owner != next_owner
                || item.eta_at != next_eta_at
                || item.blocker != next_blocker
                || item.summary != next_summary
        })
        .unwrap_or(true);

    if changed {
        sqlx::query(
            "INSERT INTO ops_incident_commands (
                alert_id, command_status, command_owner, eta_at, blocker, summary, updated_by
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (alert_id) DO UPDATE
             SET command_status = EXCLUDED.command_status,
                 command_owner = EXCLUDED.command_owner,
                 eta_at = EXCLUDED.eta_at,
                 blocker = EXCLUDED.blocker,
                 summary = EXCLUDED.summary,
                 updated_by = EXCLUDED.updated_by,
                 updated_at = NOW()",
        )
        .bind(alert_id)
        .bind(next_status.as_str())
        .bind(next_owner.as_str())
        .bind(next_eta_at)
        .bind(next_blocker.as_deref())
        .bind(next_summary.as_deref())
        .bind(actor.as_str())
        .execute(&mut *tx)
        .await?;

        let event_type = match current.as_ref() {
            None => "created",
            Some(item) if item.command_status != next_status => "status_transition",
            _ => "command_updated",
        };

        sqlx::query(
            "INSERT INTO ops_incident_command_events (
                alert_id, event_type, from_status, to_status, command_owner, eta_at, blocker, summary,
                note, actor, metadata
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(alert_id)
        .bind(event_type)
        .bind(current.as_ref().map(|item| item.command_status.as_str()))
        .bind(next_status.as_str())
        .bind(next_owner.as_str())
        .bind(next_eta_at)
        .bind(next_blocker.as_deref())
        .bind(next_summary.as_deref())
        .bind(note.as_deref())
        .bind(actor.as_str())
        .bind(json!({
            "changed": true,
            "status_changed": current.as_ref().map(|item| item.command_status != next_status).unwrap_or(true),
        }))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.incident.command.update".to_string(),
            target_type: "ops_incident_command".to_string(),
            target_id: Some(alert_id.to_string()),
            result: "success".to_string(),
            message: note.clone(),
            metadata: json!({
                "alert_id": alert_id,
                "status": next_status,
                "owner": next_owner,
                "eta_at": next_eta_at,
                "changed": changed,
            }),
        },
    )
    .await;

    let item = load_incident_item(&state.db, alert_id).await?;
    let timeline = load_incident_timeline(&state.db, alert_id).await?;
    Ok(Json(IncidentCommandDetailResponse {
        generated_at: Utc::now(),
        item,
        timeline,
    }))
}

async fn ensure_alert_exists(db: &sqlx::PgPool, alert_id: i64) -> AppResult<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM unified_alerts WHERE id = $1")
        .bind(alert_id)
        .fetch_optional(db)
        .await?;

    if exists.is_none() {
        return Err(AppError::NotFound(format!("alert {alert_id} not found")));
    }
    Ok(())
}

async fn load_incident_item(
    db: &sqlx::PgPool,
    alert_id: i64,
) -> AppResult<IncidentCommandListItem> {
    let item: Option<IncidentCommandListItem> = sqlx::query_as(
        "SELECT ua.id AS alert_id,
                ua.alert_source,
                ua.alert_key,
                ua.title,
                ua.severity,
                ua.status AS alert_status,
                ua.site,
                ua.department,
                COALESCE(c.command_status, 'triage') AS command_status,
                COALESCE(c.command_owner, 'unassigned') AS command_owner,
                c.eta_at,
                c.blocker,
                c.summary,
                COALESCE(c.updated_by, 'system') AS updated_by,
                COALESCE(c.updated_at, ua.last_seen_at) AS updated_at
         FROM unified_alerts ua
         LEFT JOIN ops_incident_commands c ON c.alert_id = ua.id
         WHERE ua.id = $1",
    )
    .bind(alert_id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("alert {alert_id} not found")))
}

async fn load_incident_timeline(
    db: &sqlx::PgPool,
    alert_id: i64,
) -> AppResult<Vec<IncidentCommandEvent>> {
    let rows: Vec<IncidentCommandEvent> = sqlx::query_as(
        "SELECT id, alert_id, event_type, from_status, to_status, command_owner,
                eta_at, blocker, summary, note, actor, created_at
         FROM ops_incident_command_events
         WHERE alert_id = $1
         ORDER BY created_at DESC, id DESC",
    )
    .bind(alert_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

fn normalize_incident_status(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        INCIDENT_STATUS_TRIAGE
        | INCIDENT_STATUS_IN_PROGRESS
        | INCIDENT_STATUS_BLOCKED
        | INCIDENT_STATUS_MITIGATED
        | INCIDENT_STATUS_POSTMORTEM => Ok(normalized),
        _ => Err(AppError::Validation(
            "status must be one of: triage, in_progress, blocked, mitigated, postmortem"
                .to_string(),
        )),
    }
}

fn normalize_required_text(field: &str, value: String, max_len: usize) -> AppResult<String> {
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

fn normalize_optional_text(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > max_len {
        return Err(AppError::Validation(format!(
            "{field} length must be <= {max_len}"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.chars().take(max_len).collect())
        }
    })
}

fn parse_optional_eta(raw: Option<String>) -> AppResult<Option<DateTime<Utc>>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let eta = DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| AppError::Validation("eta_at must use RFC3339 format".to_string()))?
        .with_timezone(&Utc);
    Ok(Some(eta))
}

fn validate_incident_transition(current_status: &str, next_status: &str) -> AppResult<()> {
    if current_status == next_status {
        return Ok(());
    }

    let allowed = match current_status {
        INCIDENT_STATUS_TRIAGE => matches!(
            next_status,
            INCIDENT_STATUS_IN_PROGRESS | INCIDENT_STATUS_BLOCKED | INCIDENT_STATUS_MITIGATED
        ),
        INCIDENT_STATUS_IN_PROGRESS => matches!(
            next_status,
            INCIDENT_STATUS_BLOCKED | INCIDENT_STATUS_MITIGATED | INCIDENT_STATUS_POSTMORTEM
        ),
        INCIDENT_STATUS_BLOCKED => {
            matches!(
                next_status,
                INCIDENT_STATUS_IN_PROGRESS | INCIDENT_STATUS_MITIGATED
            )
        }
        INCIDENT_STATUS_MITIGATED => {
            matches!(
                next_status,
                INCIDENT_STATUS_POSTMORTEM | INCIDENT_STATUS_IN_PROGRESS
            )
        }
        INCIDENT_STATUS_POSTMORTEM => next_status == INCIDENT_STATUS_IN_PROGRESS,
        _ => false,
    };

    if !allowed {
        return Err(AppError::Validation(format!(
            "invalid incident status transition: {current_status} -> {next_status}"
        )));
    }
    Ok(())
}

fn validate_incident_required_fields(
    status: &str,
    owner: &str,
    eta_at: Option<DateTime<Utc>>,
) -> AppResult<()> {
    if owner.trim().is_empty() {
        return Err(AppError::Validation("owner is required".to_string()));
    }

    if matches!(
        status,
        INCIDENT_STATUS_IN_PROGRESS | INCIDENT_STATUS_BLOCKED
    ) && eta_at.is_none()
    {
        return Err(AppError::Validation(
            "eta_at is required when status is in_progress or blocked".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        normalize_incident_status, parse_optional_eta, validate_incident_required_fields,
        validate_incident_transition,
    };

    #[test]
    fn validates_incident_status_values() {
        assert_eq!(
            normalize_incident_status("triage".to_string()).expect("triage"),
            "triage"
        );
        assert!(normalize_incident_status("invalid".to_string()).is_err());
    }

    #[test]
    fn validates_incident_transition_rules() {
        assert!(validate_incident_transition("triage", "in_progress").is_ok());
        assert!(validate_incident_transition("triage", "postmortem").is_err());
        assert!(validate_incident_transition("blocked", "mitigated").is_ok());
        assert!(validate_incident_transition("postmortem", "blocked").is_err());
    }

    #[test]
    fn validates_incident_required_fields() {
        assert!(
            validate_incident_required_fields("in_progress", "owner", Some(Utc::now())).is_ok()
        );
        assert!(validate_incident_required_fields("in_progress", "owner", None).is_err());
        assert!(validate_incident_required_fields("triage", "owner", None).is_ok());
    }

    #[test]
    fn parses_optional_eta_values() {
        let parsed = parse_optional_eta(Some("2026-03-07T10:20:30Z".to_string()))
            .expect("eta")
            .expect("present");
        assert_eq!(parsed.to_rfc3339(), "2026-03-07T10:20:30+00:00");
        assert!(parse_optional_eta(Some("not-time".to_string())).is_err());
        assert!(
            parse_optional_eta(Some("  ".to_string()))
                .expect("blank")
                .is_none()
        );
    }
}
