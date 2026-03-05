use std::collections::BTreeSet;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    audit::{actor_from_headers, write_from_headers_best_effort},
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_PLAYBOOK_KEY_LEN: usize = 64;
const MAX_CATEGORY_LEN: usize = 64;
const MAX_ASSET_REF_LEN: usize = 128;
const MAX_ACTOR_LEN: usize = 128;
const MAX_QUERY_LEN: usize = 128;
const MAX_PARAM_FIELD_KEY_LEN: usize = 64;
const MAX_PARAM_FIELD_TYPE_LEN: usize = 32;
const MAX_PARAM_FIELD_OPTIONS: usize = 128;
const MAX_PARAM_FIELDS: usize = 64;
const MAX_PLAN_STEPS: usize = 128;
const MAX_STEP_TEXT_LEN: usize = 256;
const MAX_EXECUTION_LIMIT: u32 = 200;
const DEFAULT_EXECUTION_LIMIT: u32 = 50;
const CONFIRMATION_TTL_MINUTES: i64 = 120;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playbooks", get(list_playbooks))
        .route("/playbooks/executions", get(list_playbook_executions))
        .route("/playbooks/executions/{id}", get(get_playbook_execution))
        .route(
            "/playbooks/executions/{id}/replay",
            axum::routing::post(replay_playbook_execution),
        )
        .route("/playbooks/{key}", get(get_playbook_detail))
        .route(
            "/playbooks/{key}/dry-run",
            axum::routing::post(dry_run_playbook),
        )
        .route(
            "/playbooks/{key}/execute",
            axum::routing::post(execute_playbook),
        )
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookCatalogItem {
    id: i64,
    key: String,
    name: String,
    category: String,
    risk_level: String,
    params: Value,
    description: Option<String>,
    requires_confirmation: bool,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListPlaybooksResponse {
    items: Vec<PlaybookCatalogItem>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize)]
struct PlaybookDetailResponse {
    id: i64,
    key: String,
    name: String,
    description: Option<String>,
    category: String,
    risk_level: String,
    requires_confirmation: bool,
    params: Value,
    execution_plan: Value,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookExecutionListItem {
    id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_verified: bool,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ListPlaybookExecutionsResponse {
    items: Vec<PlaybookExecutionListItem>,
    total: i64,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Serialize, FromRow)]
struct PlaybookExecutionDetail {
    id: i64,
    playbook_id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_verified: bool,
    confirmed_at: Option<DateTime<Utc>>,
    params: Value,
    planned_steps: Value,
    result: Value,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    expires_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PlaybookDryRunResponse {
    execution: PlaybookExecutionDetail,
    risk_summary: DryRunRiskSummary,
    confirmation: Option<DryRunConfirmationChallenge>,
}

#[derive(Debug, Serialize)]
struct DryRunRiskSummary {
    risk_level: String,
    requires_confirmation: bool,
    ttl_minutes: i64,
    summary: String,
}

#[derive(Debug, Serialize)]
struct DryRunConfirmationChallenge {
    token: String,
    expires_at: DateTime<Utc>,
    instruction: String,
}

#[derive(Debug, Serialize)]
struct ReplayExecutionResponse {
    mode: String,
    source_execution_id: i64,
    execution: PlaybookExecutionDetail,
    note: String,
}

#[derive(Debug, Deserialize, Default)]
struct ListPlaybooksQuery {
    category: Option<String>,
    risk_level: Option<String>,
    is_enabled: Option<bool>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct PlaybookExecutionListQuery {
    playbook_key: Option<String>,
    mode: Option<String>,
    status: Option<String>,
    actor: Option<String>,
    asset_ref: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PlaybookRunRequest {
    params: Option<Value>,
    asset_ref: Option<String>,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlaybookExecuteRequest {
    params: Option<Value>,
    asset_ref: Option<String>,
    dry_run_id: Option<i64>,
    confirmation_token: Option<String>,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ReplayExecutionRequest {
    mode: Option<String>,
    dry_run_id: Option<i64>,
    confirmation_token: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PlaybookRecord {
    id: i64,
    key: String,
    name: String,
    description: Option<String>,
    category: String,
    risk_level: String,
    requires_confirmation: bool,
    params: Value,
    execution_plan: Value,
    rbac_hint: Value,
    is_enabled: bool,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DryRunConfirmationRecord {
    id: i64,
    playbook_id: i64,
    actor: String,
    confirmation_token: Option<String>,
    confirmation_required: bool,
    expires_at: Option<DateTime<Utc>>,
    mode: String,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookParameterSchema {
    #[serde(default)]
    fields: Vec<PlaybookParameterField>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookParameterField {
    key: String,
    #[serde(rename = "type")]
    field_type: String,
    #[serde(default)]
    required: bool,
    min: Option<f64>,
    max: Option<f64>,
    max_length: Option<usize>,
    options: Option<Vec<String>>,
    default: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaybookExecutionPlan {
    #[serde(default)]
    steps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayMode {
    DryRun,
    Execute,
}

impl ReplayMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::Execute => "execute",
        }
    }
}

async fn list_playbooks(
    State(state): State<AppState>,
    Query(query): Query<ListPlaybooksQuery>,
) -> AppResult<Json<ListPlaybooksResponse>> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let category = trim_optional(query.category, MAX_CATEGORY_LEN);
    let risk_level = normalize_optional_risk_level(query.risk_level)?;
    let is_enabled = query.is_enabled.unwrap_or(true);
    let query_text = trim_optional(query.query, MAX_QUERY_LEN);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM workflow_playbooks p WHERE 1=1");
    append_playbook_filters(
        &mut count_builder,
        category.clone(),
        risk_level.clone(),
        Some(is_enabled),
        query_text.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id,
                playbook_key AS key,
                name,
                category,
                risk_level,
                parameter_schema AS params,
                description,
                requires_confirmation,
                rbac_hint,
                is_enabled,
                is_system,
                updated_at
         FROM workflow_playbooks p
         WHERE 1=1",
    );
    append_playbook_filters(
        &mut list_builder,
        category,
        risk_level,
        Some(is_enabled),
        query_text,
    );
    list_builder
        .push(" ORDER BY p.category ASC, p.risk_level DESC, p.name ASC, p.id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<PlaybookCatalogItem> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListPlaybooksResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_playbook_detail(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> AppResult<Json<PlaybookDetailResponse>> {
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;

    Ok(Json(PlaybookDetailResponse {
        id: playbook.id,
        key: playbook.key,
        name: playbook.name,
        description: playbook.description,
        category: playbook.category,
        risk_level: playbook.risk_level,
        requires_confirmation: playbook.requires_confirmation,
        params: playbook.params,
        execution_plan: playbook.execution_plan,
        rbac_hint: playbook.rbac_hint,
        is_enabled: playbook.is_enabled,
        is_system: playbook.is_system,
        created_at: playbook.created_at,
        updated_at: playbook.updated_at,
    }))
}

async fn dry_run_playbook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<PlaybookRunRequest>,
) -> AppResult<Json<PlaybookDryRunResponse>> {
    let actor = resolve_actor(&headers);
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(&playbook);
    let confirmation_token = if confirmation_required {
        Some(generate_confirmation_token())
    } else {
        None
    };

    let now = Utc::now();
    let expires_at = if confirmation_required {
        Some(now + Duration::minutes(CONFIRMATION_TTL_MINUTES))
    } else {
        None
    };
    let status = if confirmation_required {
        "planned"
    } else {
        "succeeded"
    };
    let finished_at = if confirmation_required {
        None
    } else {
        Some(now)
    };

    let result = json!({
        "mode": "dry_run",
        "summary": dry_run_risk_summary_text(playbook.risk_level.as_str(), confirmation_required),
        "requires_confirmation": confirmation_required,
        "confirmation_token": confirmation_token,
        "next_actions": [
            {
                "label": "execute_playbook",
                "api": format!("/api/v1/workflow/playbooks/{}/execute", playbook.key),
                "method": "POST"
            }
        ]
    });

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "dry_run".to_string(),
            status: status.to_string(),
            confirmation_required,
            confirmation_token: confirmation_token.clone(),
            confirmation_verified: !confirmation_required,
            confirmed_at: None,
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id: None,
            expires_at,
            finished_at,
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.dry_run",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "playbook_key": playbook.key,
            "risk_level": playbook.risk_level,
            "confirmation_required": confirmation_required,
            "mode": "dry_run"
        }),
    )
    .await;

    let confirmation = execution_confirmation_challenge(&execution);

    Ok(Json(PlaybookDryRunResponse {
        execution,
        risk_summary: DryRunRiskSummary {
            risk_level: playbook.risk_level.clone(),
            requires_confirmation: confirmation_required,
            ttl_minutes: CONFIRMATION_TTL_MINUTES,
            summary: dry_run_risk_summary_text(playbook.risk_level.as_str(), confirmation_required),
        },
        confirmation,
    }))
}

async fn execute_playbook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<PlaybookExecuteRequest>,
) -> AppResult<Json<PlaybookExecutionDetail>> {
    let actor = resolve_actor(&headers);
    let normalized_key = normalize_playbook_key(key)?;
    let playbook = load_playbook_by_key(&state.db, &normalized_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(&playbook);
    if confirmation_required {
        let dry_run_id = payload.dry_run_id.ok_or_else(|| {
            AppError::Validation(
                "dry_run_id is required for high-risk playbook execution".to_string(),
            )
        })?;
        let confirmation_token = required_trimmed_token(payload.confirmation_token)?;

        verify_and_consume_confirmation(
            &state.db,
            dry_run_id,
            playbook.id,
            &actor,
            &confirmation_token,
        )
        .await?;
    }

    let now = Utc::now();
    let result = json!({
        "mode": "execute",
        "summary": "Playbook execution completed with audit trail.",
        "next_actions": [
            {
                "label": "open_workflow_page",
                "href": "#/workflow"
            },
            {
                "label": "open_alert_center",
                "href": "#/alerts"
            }
        ]
    });

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "execute".to_string(),
            status: "succeeded".to_string(),
            confirmation_required,
            confirmation_token: None,
            confirmation_verified: !confirmation_required,
            confirmed_at: if confirmation_required {
                Some(now)
            } else {
                None
            },
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id: None,
            expires_at: None,
            finished_at: Some(now),
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        &headers,
        "workflow.playbook.execute",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "playbook_key": playbook.key,
            "risk_level": playbook.risk_level,
            "mode": "execute",
            "confirmation_required": confirmation_required
        }),
    )
    .await;

    Ok(Json(execution))
}

async fn list_playbook_executions(
    State(state): State<AppState>,
    Query(query): Query<PlaybookExecutionListQuery>,
) -> AppResult<Json<ListPlaybookExecutionsResponse>> {
    let playbook_key = query.playbook_key.map(normalize_playbook_key).transpose()?;
    let mode = normalize_optional_mode(query.mode)?;
    let status = normalize_optional_execution_status(query.status)?;
    let actor = trim_optional(query.actor, MAX_ACTOR_LEN);
    let asset_ref = trim_optional(query.asset_ref, MAX_ASSET_REF_LEN);

    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM workflow_playbook_executions e WHERE 1=1");
    append_execution_filters(
        &mut count_builder,
        playbook_key.clone(),
        mode.clone(),
        status.clone(),
        actor.clone(),
        asset_ref.clone(),
    );
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT
            e.id,
            e.playbook_key,
            e.playbook_name,
            e.category,
            e.risk_level,
            e.actor,
            e.asset_ref,
            e.mode,
            e.status,
            e.confirmation_required,
            e.confirmation_verified,
            e.related_ticket_id,
            e.related_alert_id,
            e.replay_of_execution_id,
            e.created_at,
            e.finished_at
         FROM workflow_playbook_executions e
         WHERE 1=1",
    );

    append_execution_filters(
        &mut list_builder,
        playbook_key,
        mode,
        status,
        actor,
        asset_ref,
    );
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<PlaybookExecutionListItem> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    Ok(Json(ListPlaybookExecutionsResponse {
        items,
        total,
        limit: limit as u32,
        offset: offset as u32,
    }))
}

async fn get_playbook_execution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<PlaybookExecutionDetail>> {
    let execution = load_playbook_execution_detail(&state.db, id).await?;
    Ok(Json(execution))
}

async fn replay_playbook_execution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<ReplayExecutionRequest>,
) -> AppResult<Json<ReplayExecutionResponse>> {
    let actor = resolve_actor(&headers);
    let source = load_playbook_execution_detail(&state.db, id).await?;

    let replay_mode = parse_replay_mode(payload.mode)?;
    let playbook = load_playbook_by_key(&state.db, &source.playbook_key).await?;
    ensure_playbook_enabled(&playbook)?;

    let execution = match replay_mode {
        ReplayMode::DryRun => {
            let request = PlaybookRunRequest {
                params: Some(source.params.clone()),
                asset_ref: source.asset_ref.clone(),
                related_ticket_id: source.related_ticket_id,
                related_alert_id: source.related_alert_id,
            };
            run_replay_dry_run(
                &state,
                &headers,
                &playbook,
                request,
                Some(source.id),
                actor.clone(),
            )
            .await?
        }
        ReplayMode::Execute => {
            let request = PlaybookExecuteRequest {
                params: Some(source.params.clone()),
                asset_ref: source.asset_ref.clone(),
                dry_run_id: payload.dry_run_id,
                confirmation_token: payload.confirmation_token,
                related_ticket_id: source.related_ticket_id,
                related_alert_id: source.related_alert_id,
            };
            run_replay_execute(
                &state,
                &headers,
                &playbook,
                request,
                Some(source.id),
                actor.clone(),
            )
            .await?
        }
    };

    Ok(Json(ReplayExecutionResponse {
        mode: replay_mode.as_str().to_string(),
        source_execution_id: source.id,
        execution,
        note:
            "Replay request accepted. High-risk execute replay still requires dry-run confirmation."
                .to_string(),
    }))
}

async fn run_replay_dry_run(
    state: &AppState,
    headers: &HeaderMap,
    playbook: &PlaybookRecord,
    payload: PlaybookRunRequest,
    replay_of_execution_id: Option<i64>,
    actor: String,
) -> AppResult<PlaybookExecutionDetail> {
    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(playbook);
    let now = Utc::now();
    let expires_at = if confirmation_required {
        Some(now + Duration::minutes(CONFIRMATION_TTL_MINUTES))
    } else {
        None
    };
    let confirmation_token = if confirmation_required {
        Some(generate_confirmation_token())
    } else {
        None
    };

    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "dry_run".to_string(),
            status: if confirmation_required {
                "planned".to_string()
            } else {
                "succeeded".to_string()
            },
            confirmation_required,
            confirmation_token: confirmation_token.clone(),
            confirmation_verified: !confirmation_required,
            confirmed_at: None,
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result: json!({
                "mode": "dry_run",
                "summary": "Replay dry-run generated from historical execution.",
                "confirmation_token": confirmation_token,
            }),
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at: if confirmation_required {
                None
            } else {
                Some(now)
            },
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        headers,
        "workflow.playbook.replay",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "mode": "dry_run",
            "playbook_key": playbook.key,
            "replay_of_execution_id": replay_of_execution_id,
        }),
    )
    .await;

    Ok(execution)
}

async fn run_replay_execute(
    state: &AppState,
    headers: &HeaderMap,
    playbook: &PlaybookRecord,
    payload: PlaybookExecuteRequest,
    replay_of_execution_id: Option<i64>,
    actor: String,
) -> AppResult<PlaybookExecutionDetail> {
    let schema = parse_parameter_schema(playbook.params.clone())?;
    let normalized_params = normalize_playbook_params(payload.params, &schema)?;
    let asset_ref = trim_optional(payload.asset_ref, MAX_ASSET_REF_LEN);
    let related_ticket_id =
        normalize_optional_positive_id(payload.related_ticket_id, "related_ticket_id")?;
    let related_alert_id =
        normalize_optional_positive_id(payload.related_alert_id, "related_alert_id")?;
    let planned_steps = parse_execution_plan_steps(playbook.execution_plan.clone())?;

    let confirmation_required = playbook_requires_confirmation(playbook);
    if confirmation_required {
        let dry_run_id = payload.dry_run_id.ok_or_else(|| {
            AppError::Validation(
                "dry_run_id is required for high-risk playbook replay execution".to_string(),
            )
        })?;
        let confirmation_token = required_trimmed_token(payload.confirmation_token)?;

        verify_and_consume_confirmation(
            &state.db,
            dry_run_id,
            playbook.id,
            &actor,
            &confirmation_token,
        )
        .await?;
    }

    let now = Utc::now();
    let execution = insert_playbook_execution(
        &state.db,
        PlaybookExecutionInsertInput {
            playbook_id: playbook.id,
            playbook_key: playbook.key.clone(),
            playbook_name: playbook.name.clone(),
            category: playbook.category.clone(),
            risk_level: playbook.risk_level.clone(),
            actor: actor.clone(),
            asset_ref,
            mode: "execute".to_string(),
            status: "succeeded".to_string(),
            confirmation_required,
            confirmation_token: None,
            confirmation_verified: !confirmation_required,
            confirmed_at: if confirmation_required {
                Some(now)
            } else {
                None
            },
            params: normalized_params,
            planned_steps: Value::Array(
                planned_steps
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
            result: json!({
                "mode": "execute",
                "summary": "Replay execute completed with the same validated parameters.",
            }),
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at: None,
            finished_at: Some(now),
        },
    )
    .await?;

    write_from_headers_best_effort(
        &state.db,
        headers,
        "workflow.playbook.replay",
        "workflow_playbook",
        Some(execution.id.to_string()),
        "success",
        None,
        json!({
            "mode": "execute",
            "playbook_key": playbook.key,
            "replay_of_execution_id": replay_of_execution_id,
        }),
    )
    .await;

    Ok(execution)
}

async fn load_playbook_by_key(db: &sqlx::PgPool, key: &str) -> AppResult<PlaybookRecord> {
    let item: Option<PlaybookRecord> = sqlx::query_as(
        "SELECT id,
                playbook_key AS key,
                name,
                description,
                category,
                risk_level,
                requires_confirmation,
                parameter_schema AS params,
                execution_plan,
                rbac_hint,
                is_enabled,
                is_system,
                created_at,
                updated_at
         FROM workflow_playbooks
         WHERE playbook_key = $1",
    )
    .bind(key)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("playbook '{key}' not found")))
}

async fn load_playbook_execution_detail(
    db: &sqlx::PgPool,
    id: i64,
) -> AppResult<PlaybookExecutionDetail> {
    let item: Option<PlaybookExecutionDetail> = sqlx::query_as(
        "SELECT
            id,
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_verified,
            confirmed_at,
            params_json AS params,
            planned_steps,
            result_json AS result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at,
            created_at,
            updated_at
         FROM workflow_playbook_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("playbook execution {id} not found")))
}

fn append_playbook_filters(
    builder: &mut QueryBuilder<Postgres>,
    category: Option<String>,
    risk_level: Option<String>,
    is_enabled: Option<bool>,
    query_text: Option<String>,
) {
    if let Some(category) = category {
        builder.push(" AND p.category = ").push_bind(category);
    }
    if let Some(risk_level) = risk_level {
        builder.push(" AND p.risk_level = ").push_bind(risk_level);
    }
    if let Some(is_enabled) = is_enabled {
        builder.push(" AND p.is_enabled = ").push_bind(is_enabled);
    }
    if let Some(query_text) = query_text {
        let like = format!("%{query_text}%");
        builder.push(" AND (");
        builder
            .push("p.playbook_key ILIKE ")
            .push_bind(like.clone());
        builder.push(" OR p.name ILIKE ").push_bind(like.clone());
        builder
            .push(" OR COALESCE(p.description, '') ILIKE ")
            .push_bind(like);
        builder.push(")");
    }
}

fn append_execution_filters(
    builder: &mut QueryBuilder<Postgres>,
    playbook_key: Option<String>,
    mode: Option<String>,
    status: Option<String>,
    actor: Option<String>,
    asset_ref: Option<String>,
) {
    if let Some(playbook_key) = playbook_key {
        builder
            .push(" AND e.playbook_key = ")
            .push_bind(playbook_key);
    }
    if let Some(mode) = mode {
        builder.push(" AND e.mode = ").push_bind(mode);
    }
    if let Some(status) = status {
        builder.push(" AND e.status = ").push_bind(status);
    }
    if let Some(actor) = actor {
        builder
            .push(" AND e.actor ILIKE ")
            .push_bind(format!("%{actor}%"));
    }
    if let Some(asset_ref) = asset_ref {
        builder
            .push(" AND e.asset_ref ILIKE ")
            .push_bind(format!("%{asset_ref}%"));
    }
}

async fn verify_and_consume_confirmation(
    db: &sqlx::PgPool,
    dry_run_id: i64,
    playbook_id: i64,
    actor: &str,
    confirmation_token: &str,
) -> AppResult<()> {
    if dry_run_id <= 0 {
        return Err(AppError::Validation(
            "dry_run_id must be a positive integer".to_string(),
        ));
    }

    let record: Option<DryRunConfirmationRecord> = sqlx::query_as(
        "SELECT
            id,
            playbook_id,
            actor,
            confirmation_token,
            confirmation_required,
            expires_at,
            mode,
            status
         FROM workflow_playbook_executions
         WHERE id = $1",
    )
    .bind(dry_run_id)
    .fetch_optional(db)
    .await?;

    let record = record
        .ok_or_else(|| AppError::Validation(format!("dry-run execution {dry_run_id} not found")))?;

    let now = Utc::now();
    match validate_confirmation_transition(&record, playbook_id, actor, confirmation_token, now)? {
        ConfirmationDecision::Allow => {}
        ConfirmationDecision::Expired => {
            sqlx::query(
                "UPDATE workflow_playbook_executions
                 SET status = 'expired', updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(record.id)
            .execute(db)
            .await?;
            return Err(AppError::Validation(
                "dry-run confirmation has expired; create a new dry-run".to_string(),
            ));
        }
    }

    sqlx::query(
        "UPDATE workflow_playbook_executions
         SET confirmation_verified = TRUE,
             confirmed_at = NOW(),
             status = 'succeeded',
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(record.id)
    .execute(db)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmationDecision {
    Allow,
    Expired,
}

fn validate_confirmation_transition(
    record: &DryRunConfirmationRecord,
    playbook_id: i64,
    actor: &str,
    confirmation_token: &str,
    now: DateTime<Utc>,
) -> AppResult<ConfirmationDecision> {
    if record.mode != "dry_run" {
        return Err(AppError::Validation(format!(
            "execution {} is not a dry-run record",
            record.id
        )));
    }
    if record.playbook_id != playbook_id {
        return Err(AppError::Validation(
            "dry-run execution belongs to another playbook".to_string(),
        ));
    }
    if !record.actor.eq_ignore_ascii_case(actor) {
        return Err(AppError::Forbidden(
            "dry-run confirmation can only be consumed by the same actor".to_string(),
        ));
    }
    if !record.confirmation_required {
        return Err(AppError::Validation(
            "dry-run confirmation is not required for this playbook".to_string(),
        ));
    }
    if !matches!(record.status.as_str(), "planned" | "succeeded") {
        return Err(AppError::Validation(format!(
            "dry-run execution status '{}' cannot be used for confirmation",
            record.status
        )));
    }
    if let Some(expires_at) = record.expires_at {
        if expires_at < now {
            return Ok(ConfirmationDecision::Expired);
        }
    }
    let expected_token = record
        .confirmation_token
        .as_deref()
        .ok_or_else(|| AppError::Validation("dry-run confirmation token is missing".to_string()))?;
    if expected_token != confirmation_token {
        return Err(AppError::Validation(
            "confirmation_token does not match dry-run challenge".to_string(),
        ));
    }

    Ok(ConfirmationDecision::Allow)
}

struct PlaybookExecutionInsertInput {
    playbook_id: i64,
    playbook_key: String,
    playbook_name: String,
    category: String,
    risk_level: String,
    actor: String,
    asset_ref: Option<String>,
    mode: String,
    status: String,
    confirmation_required: bool,
    confirmation_token: Option<String>,
    confirmation_verified: bool,
    confirmed_at: Option<DateTime<Utc>>,
    params: Value,
    planned_steps: Value,
    result: Value,
    related_ticket_id: Option<i64>,
    related_alert_id: Option<i64>,
    replay_of_execution_id: Option<i64>,
    expires_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

async fn insert_playbook_execution(
    db: &sqlx::PgPool,
    input: PlaybookExecutionInsertInput,
) -> AppResult<PlaybookExecutionDetail> {
    let item: PlaybookExecutionDetail = sqlx::query_as(
        "INSERT INTO workflow_playbook_executions (
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_token,
            confirmation_verified,
            confirmed_at,
            params_json,
            planned_steps,
            result_json,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at
         )
         VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
            $11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21
         )
         RETURNING
            id,
            playbook_id,
            playbook_key,
            playbook_name,
            category,
            risk_level,
            actor,
            asset_ref,
            mode,
            status,
            confirmation_required,
            confirmation_verified,
            confirmed_at,
            params_json AS params,
            planned_steps,
            result_json AS result,
            related_ticket_id,
            related_alert_id,
            replay_of_execution_id,
            expires_at,
            finished_at,
            created_at,
            updated_at",
    )
    .bind(input.playbook_id)
    .bind(input.playbook_key)
    .bind(input.playbook_name)
    .bind(input.category)
    .bind(input.risk_level)
    .bind(input.actor)
    .bind(input.asset_ref)
    .bind(input.mode)
    .bind(input.status)
    .bind(input.confirmation_required)
    .bind(input.confirmation_token)
    .bind(input.confirmation_verified)
    .bind(input.confirmed_at)
    .bind(input.params)
    .bind(input.planned_steps)
    .bind(input.result)
    .bind(input.related_ticket_id)
    .bind(input.related_alert_id)
    .bind(input.replay_of_execution_id)
    .bind(input.expires_at)
    .bind(input.finished_at)
    .fetch_one(db)
    .await?;

    Ok(item)
}

fn parse_parameter_schema(schema: Value) -> AppResult<PlaybookParameterSchema> {
    let schema: PlaybookParameterSchema = serde_json::from_value(schema).map_err(|err| {
        AppError::Validation(format!("playbook parameter schema is invalid: {err}"))
    })?;

    if schema.fields.len() > MAX_PARAM_FIELDS {
        return Err(AppError::Validation(format!(
            "playbook parameter schema fields must be <= {MAX_PARAM_FIELDS}"
        )));
    }

    let mut seen = BTreeSet::new();
    for field in &schema.fields {
        let key = normalize_param_field_key(field.key.clone())?;
        if !seen.insert(key.clone()) {
            return Err(AppError::Validation(format!(
                "playbook parameter field '{key}' is duplicated"
            )));
        }

        let field_type = normalize_field_type(field.field_type.clone())?;
        if field_type == "enum" {
            let Some(options) = field.options.as_ref() else {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' type enum requires options"
                )));
            };
            if options.is_empty() || options.len() > MAX_PARAM_FIELD_OPTIONS {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' enum options must be between 1 and {MAX_PARAM_FIELD_OPTIONS}"
                )));
            }
        }

        if let (Some(min), Some(max)) = (field.min, field.max) {
            if min > max {
                return Err(AppError::Validation(format!(
                    "parameter field '{key}' has min > max"
                )));
            }
        }
    }

    Ok(schema)
}

fn parse_execution_plan_steps(plan: Value) -> AppResult<Vec<String>> {
    let plan: PlaybookExecutionPlan = serde_json::from_value(plan).map_err(|err| {
        AppError::Validation(format!("playbook execution plan is invalid: {err}"))
    })?;

    if plan.steps.is_empty() {
        return Err(AppError::Validation(
            "playbook execution plan must include at least one step".to_string(),
        ));
    }
    if plan.steps.len() > MAX_PLAN_STEPS {
        return Err(AppError::Validation(format!(
            "playbook execution plan steps must be <= {MAX_PLAN_STEPS}"
        )));
    }

    let mut normalized_steps = Vec::with_capacity(plan.steps.len());
    for raw_step in plan.steps {
        let step = raw_step.trim();
        if step.is_empty() {
            return Err(AppError::Validation(
                "playbook execution plan contains an empty step".to_string(),
            ));
        }
        if step.chars().count() > MAX_STEP_TEXT_LEN {
            return Err(AppError::Validation(format!(
                "playbook execution step length must be <= {MAX_STEP_TEXT_LEN}"
            )));
        }
        normalized_steps.push(step.to_string());
    }

    Ok(normalized_steps)
}

fn normalize_playbook_params(
    params: Option<Value>,
    schema: &PlaybookParameterSchema,
) -> AppResult<Value> {
    let raw = params.unwrap_or_else(|| json!({}));
    let raw_object = raw
        .as_object()
        .ok_or_else(|| AppError::Validation("playbook params must be a JSON object".to_string()))?;

    let mut allowed = BTreeSet::new();
    let mut output = JsonMap::new();

    for field in &schema.fields {
        let key = normalize_param_field_key(field.key.clone())?;
        allowed.insert(key.clone());

        let input_value = raw_object
            .get(&key)
            .cloned()
            .or_else(|| field.default.clone());

        if input_value.is_none() {
            if field.required {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' is required"
                )));
            }
            continue;
        }

        let value = input_value.expect("checked is_some");
        let normalized = validate_field_value(&key, field, value)?;
        output.insert(key, normalized);
    }

    let unknown_keys: Vec<String> = raw_object
        .keys()
        .filter(|key| !allowed.contains(*key))
        .map(|key| key.to_string())
        .collect();

    if !unknown_keys.is_empty() {
        return Err(AppError::Validation(format!(
            "unknown playbook params: {}",
            unknown_keys.join(", ")
        )));
    }

    Ok(Value::Object(output))
}

fn validate_field_value(
    key: &str,
    field: &PlaybookParameterField,
    value: Value,
) -> AppResult<Value> {
    let field_type = normalize_field_type(field.field_type.clone())?;

    match field_type.as_str() {
        "string" => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be a string"))
            })?;
            let text = text.trim();
            if field.required && text.is_empty() {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' cannot be empty"
                )));
            }
            if let Some(max_length) = field.max_length {
                if text.chars().count() > max_length {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' length must be <= {max_length}"
                    )));
                }
            }
            Ok(Value::String(text.to_string()))
        }
        "integer" => {
            let number = value.as_i64().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be an integer"))
            })?;
            if let Some(min) = field.min {
                if (number as f64) < min {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be >= {min}"
                    )));
                }
            }
            if let Some(max) = field.max {
                if (number as f64) > max {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be <= {max}"
                    )));
                }
            }
            Ok(Value::Number(number.into()))
        }
        "number" => {
            let number = value.as_f64().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be numeric"))
            })?;
            if let Some(min) = field.min {
                if number < min {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be >= {min}"
                    )));
                }
            }
            if let Some(max) = field.max {
                if number > max {
                    return Err(AppError::Validation(format!(
                        "playbook param '{key}' must be <= {max}"
                    )));
                }
            }
            Ok(json!(number))
        }
        "boolean" => {
            let bool_value = value.as_bool().ok_or_else(|| {
                AppError::Validation(format!("playbook param '{key}' must be boolean"))
            })?;
            Ok(Value::Bool(bool_value))
        }
        "enum" => {
            let text = value.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "playbook param '{key}' enum value must be a string"
                ))
            })?;
            let normalized = text.trim();
            let options = field.options.as_ref().ok_or_else(|| {
                AppError::Validation(format!(
                    "playbook parameter schema for '{key}' is missing enum options"
                ))
            })?;
            let options_set = options
                .iter()
                .map(|item| item.trim().to_string())
                .collect::<BTreeSet<_>>();
            if !options_set.contains(normalized) {
                return Err(AppError::Validation(format!(
                    "playbook param '{key}' must be one of: {}",
                    options_set.into_iter().collect::<Vec<_>>().join(", ")
                )));
            }
            Ok(Value::String(normalized.to_string()))
        }
        _ => Err(AppError::Validation(format!(
            "playbook parameter field '{key}' has unsupported type '{field_type}'"
        ))),
    }
}

fn normalize_param_field_key(key: String) -> AppResult<String> {
    let key = key.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(AppError::Validation(
            "playbook parameter field key cannot be empty".to_string(),
        ));
    }
    if key.len() > MAX_PARAM_FIELD_KEY_LEN {
        return Err(AppError::Validation(format!(
            "playbook parameter field key length must be <= {MAX_PARAM_FIELD_KEY_LEN}"
        )));
    }
    if !key
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "playbook parameter field key can only include [a-z0-9_-]".to_string(),
        ));
    }
    Ok(key)
}

fn normalize_field_type(field_type: String) -> AppResult<String> {
    let normalized = field_type.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "playbook parameter field type is required".to_string(),
        ));
    }
    if normalized.len() > MAX_PARAM_FIELD_TYPE_LEN {
        return Err(AppError::Validation(format!(
            "playbook parameter field type length must be <= {MAX_PARAM_FIELD_TYPE_LEN}"
        )));
    }
    if !matches!(
        normalized.as_str(),
        "string" | "integer" | "number" | "boolean" | "enum"
    ) {
        return Err(AppError::Validation(format!(
            "unsupported parameter field type '{normalized}'"
        )));
    }
    Ok(normalized)
}

fn normalize_optional_risk_level(value: Option<String>) -> AppResult<Option<String>> {
    value.map(normalize_risk_level).transpose()
}

fn normalize_risk_level(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "critical" => Ok(normalized),
        _ => Err(AppError::Validation(
            "risk_level must be one of: low, medium, high, critical".to_string(),
        )),
    }
}

fn normalize_playbook_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("playbook key is required".to_string()));
    }
    if normalized.len() > MAX_PLAYBOOK_KEY_LEN {
        return Err(AppError::Validation(format!(
            "playbook key length must be <= {MAX_PLAYBOOK_KEY_LEN}"
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "playbook key can only include lowercase letters, numbers, '_' and '-'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_optional_mode(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "dry_run" | "execute" => Ok(normalized),
                _ => Err(AppError::Validation(
                    "mode must be one of: dry_run, execute".to_string(),
                )),
            }
        })
        .transpose()
}

fn normalize_optional_execution_status(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "planned" | "succeeded" | "failed" | "blocked" | "expired" => Ok(normalized),
                _ => Err(AppError::Validation(
                    "status must be one of: planned, succeeded, failed, blocked, expired"
                        .to_string(),
                )),
            }
        })
        .transpose()
}

fn normalize_optional_positive_id(value: Option<i64>, field: &str) -> AppResult<Option<i64>> {
    match value {
        Some(value) if value <= 0 => Err(AppError::Validation(format!("{field} must be positive"))),
        _ => Ok(value),
    }
}

fn parse_replay_mode(value: Option<String>) -> AppResult<ReplayMode> {
    let Some(raw) = value else {
        return Ok(ReplayMode::DryRun);
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "dry_run" => Ok(ReplayMode::DryRun),
        "execute" => Ok(ReplayMode::Execute),
        _ => Err(AppError::Validation(
            "replay mode must be one of: dry_run, execute".to_string(),
        )),
    }
}

fn required_trimmed_token(value: Option<String>) -> AppResult<String> {
    let token = value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "confirmation_token is required for high-risk playbook execution".to_string(),
            )
        })?;

    if token.len() > 128 {
        return Err(AppError::Validation(
            "confirmation_token length must be <= 128".to_string(),
        ));
    }

    Ok(token)
}

fn playbook_requires_confirmation(playbook: &PlaybookRecord) -> bool {
    if playbook.requires_confirmation {
        return true;
    }

    matches!(playbook.risk_level.as_str(), "high" | "critical")
}

fn dry_run_risk_summary_text(risk_level: &str, confirmation_required: bool) -> String {
    if confirmation_required {
        format!("Risk level '{risk_level}' requires explicit confirmation after dry-run preview.")
    } else {
        format!("Risk level '{risk_level}' can execute directly after optional dry-run.")
    }
}

fn execution_confirmation_challenge(
    execution: &PlaybookExecutionDetail,
) -> Option<DryRunConfirmationChallenge> {
    let token = execution
        .result
        .get("confirmation_token")
        .and_then(Value::as_str)
        .map(|value| value.to_string());

    let token = token.or_else(|| {
        execution
            .confirmation_required
            .then(|| format!("dry-run-{}", execution.id))
    });

    if !execution.confirmation_required {
        return None;
    }

    Some(DryRunConfirmationChallenge {
        token: token.unwrap_or_else(|| format!("dry-run-{}", execution.id)),
        expires_at: execution.expires_at.unwrap_or_else(Utc::now),
        instruction: "Use this token as confirmation_token with dry_run_id on execute endpoint."
            .to_string(),
    })
}

fn generate_confirmation_token() -> String {
    let uuid = Uuid::new_v4().simple().to_string().to_ascii_uppercase();
    format!("PBK-{}", &uuid[..8])
}

fn ensure_playbook_enabled(playbook: &PlaybookRecord) -> AppResult<()> {
    if !playbook.is_enabled {
        return Err(AppError::Validation(format!(
            "playbook '{}' is disabled",
            playbook.key
        )));
    }
    Ok(())
}

fn resolve_actor(headers: &HeaderMap) -> String {
    actor_from_headers(headers)
        .filter(|value| !value.trim().is_empty())
        .map(|value| trim_to_len(value.trim(), MAX_ACTOR_LEN))
        .unwrap_or_else(|| "unknown".to_string())
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trim_to_len(trimmed, max_len))
        }
    })
}

fn trim_to_len(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }

    value.chars().take(max_len).collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use serde_json::json;

    use super::{
        ConfirmationDecision, DryRunConfirmationRecord, PlaybookParameterSchema,
        dry_run_risk_summary_text, normalize_field_type, normalize_playbook_key,
        normalize_playbook_params, parse_parameter_schema, validate_confirmation_transition,
    };

    fn sample_schema() -> PlaybookParameterSchema {
        parse_parameter_schema(json!({
            "fields": [
                {"key": "asset_ref", "type": "string", "required": true, "max_length": 10},
                {"key": "grace_seconds", "type": "integer", "required": false, "default": 30, "min": 0, "max": 600},
                {"key": "force", "type": "boolean", "required": false, "default": false},
                {"key": "mode", "type": "enum", "required": false, "options": ["safe", "force"], "default": "safe"}
            ]
        }))
        .expect("schema")
    }

    #[test]
    fn normalizes_playbook_key() {
        assert_eq!(
            normalize_playbook_key(" Restart-Service-Safe ".to_string()).expect("key"),
            "restart-service-safe"
        );
        assert!(normalize_playbook_key("bad key".to_string()).is_err());
    }

    #[test]
    fn validates_supported_field_types() {
        assert_eq!(
            normalize_field_type(" String ".to_string()).expect("field type"),
            "string"
        );
        assert!(normalize_field_type("array".to_string()).is_err());
    }

    #[test]
    fn validates_required_and_unknown_params() {
        let schema = sample_schema();
        let result = normalize_playbook_params(Some(json!({"grace_seconds": 10})), &schema);
        assert!(result.is_err());

        let result =
            normalize_playbook_params(Some(json!({"asset_ref":"srv-a","unexpected":"x"})), &schema);
        assert!(result.is_err());
    }

    #[test]
    fn applies_defaults_and_type_validation() {
        let schema = sample_schema();
        let normalized = normalize_playbook_params(Some(json!({"asset_ref": "srv-a"})), &schema)
            .expect("normalized");

        assert_eq!(
            normalized.get("asset_ref").and_then(|v| v.as_str()),
            Some("srv-a")
        );
        assert_eq!(
            normalized.get("grace_seconds").and_then(|v| v.as_i64()),
            Some(30)
        );
        assert_eq!(
            normalized.get("force").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            normalized.get("mode").and_then(|v| v.as_str()),
            Some("safe")
        );

        let invalid = normalize_playbook_params(
            Some(json!({"asset_ref": "srv-a", "grace_seconds": "oops"})),
            &schema,
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn risk_summary_text_mentions_confirmation_when_required() {
        let summary = dry_run_risk_summary_text("high", true);
        assert!(summary.contains("requires explicit confirmation"));

        let summary = dry_run_risk_summary_text("low", false);
        assert!(summary.contains("can execute directly"));
    }

    #[test]
    fn confirmation_transition_accepts_valid_request() {
        let now = Utc::now();
        let record = DryRunConfirmationRecord {
            id: 100,
            playbook_id: 9,
            actor: "operator-a".to_string(),
            confirmation_token: Some("PBK-ABC12345".to_string()),
            confirmation_required: true,
            expires_at: Some(now + Duration::minutes(30)),
            mode: "dry_run".to_string(),
            status: "planned".to_string(),
        };

        let result =
            validate_confirmation_transition(&record, 9, "operator-a", "PBK-ABC12345", now);
        assert!(result.is_ok());
    }

    #[test]
    fn confirmation_transition_rejects_wrong_token_or_actor_and_marks_expired() {
        let now = Utc::now();
        let base_record = DryRunConfirmationRecord {
            id: 101,
            playbook_id: 9,
            actor: "operator-a".to_string(),
            confirmation_token: Some("PBK-ABC12345".to_string()),
            confirmation_required: true,
            expires_at: Some(now + Duration::minutes(5)),
            mode: "dry_run".to_string(),
            status: "planned".to_string(),
        };

        let wrong_token =
            validate_confirmation_transition(&base_record, 9, "operator-a", "PBK-WRONG", now);
        assert!(wrong_token.is_err());

        let wrong_actor =
            validate_confirmation_transition(&base_record, 9, "viewer-a", "PBK-ABC12345", now);
        assert!(wrong_actor.is_err());

        let expired_record = DryRunConfirmationRecord {
            expires_at: Some(now - Duration::minutes(1)),
            ..base_record
        };
        let expired =
            validate_confirmation_transition(&expired_record, 9, "operator-a", "PBK-ABC12345", now);
        assert!(matches!(expired, Ok(ConfirmationDecision::Expired)));
    }
}
