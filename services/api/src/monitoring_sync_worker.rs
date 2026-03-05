use std::{
    env,
    time::{Duration, Instant},
};

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};
use sqlx::FromRow;
use tokio::{task::JoinSet, time::sleep};
use tracing::{error, info, warn};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    cmdb::monitoring_sync::is_eligible_asset_class,
    error::{AppError, AppResult},
    secrets::resolve_monitoring_secret,
    state::AppState,
};

const WORKER_ACTOR: &str = "monitoring-sync-worker";
const STATUS_PENDING: &str = "pending";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCESS: &str = "success";
const STATUS_FAILED: &str = "failed";
const STATUS_DEAD_LETTER: &str = "dead_letter";
const STATUS_SKIPPED: &str = "skipped";

pub fn start(state: AppState) {
    if !worker_enabled() {
        info!("monitoring sync worker disabled by MONITORING_SYNC_WORKER_ENABLED");
        return;
    }

    tokio::spawn(async move {
        if let Err(err) = run_loop(state).await {
            error!(error = %err, "monitoring sync worker terminated unexpectedly");
        }
    });
}

async fn run_loop(state: AppState) -> AppResult<()> {
    let poll_interval = Duration::from_secs(poll_interval_seconds());
    let batch_size = worker_batch_size();
    let max_parallel = worker_max_parallel();
    info!(
        poll_interval_seconds = poll_interval.as_secs(),
        batch_size, max_parallel, "monitoring sync worker started"
    );

    loop {
        match claim_pending_jobs(&state, batch_size).await {
            Ok(jobs) => {
                if jobs.is_empty() {
                    sleep(poll_interval).await;
                    continue;
                }
                process_claimed_jobs(&state, jobs, max_parallel).await?;
            }
            Err(err) => {
                warn!(error = %err, "monitoring sync worker loop error");
                sleep(poll_interval).await;
            }
        }
    }
}

async fn claim_pending_jobs(state: &AppState, batch_size: i64) -> AppResult<Vec<SyncJobRecord>> {
    let mut tx = state.db.begin().await?;
    let mut jobs: Vec<SyncJobRecord> = sqlx::query_as(
        "SELECT id, asset_id, trigger_source, attempt, max_attempts
         FROM cmdb_monitoring_sync_jobs
         WHERE status = 'pending'
           AND run_after <= NOW()
         ORDER BY run_after ASC, id ASC
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(batch_size)
    .fetch_all(&mut *tx)
    .await?;

    if jobs.is_empty() {
        tx.rollback().await?;
        return Ok(Vec::new());
    }

    for job in &mut jobs {
        let next_attempt = job.attempt + 1;
        sqlx::query(
            "UPDATE cmdb_monitoring_sync_jobs
             SET status = $2,
                 attempt = $3,
                 started_at = COALESCE(started_at, NOW()),
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(job.id)
        .bind(STATUS_RUNNING)
        .bind(next_attempt)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO cmdb_monitoring_bindings (asset_id, source_system, last_sync_status, last_sync_message, last_sync_at, mapping)
             VALUES ($1, 'zabbix', $2, $3, NOW(), '{}'::jsonb)
             ON CONFLICT (asset_id) DO UPDATE
             SET last_sync_status = EXCLUDED.last_sync_status,
                 last_sync_message = EXCLUDED.last_sync_message,
                 last_sync_at = NOW(),
                 updated_at = NOW()",
        )
        .bind(job.asset_id)
        .bind(STATUS_RUNNING)
        .bind(format!("sync job #{} running (attempt {})", job.id, next_attempt))
        .execute(&mut *tx)
        .await?;
        job.attempt = next_attempt;
    }

    tx.commit().await?;
    Ok(jobs)
}

async fn process_claimed_jobs(
    state: &AppState,
    jobs: Vec<SyncJobRecord>,
    max_parallel: usize,
) -> AppResult<()> {
    let depth_before = load_queue_depth(&state.db).await.unwrap_or_default();
    let batch_started = Instant::now();
    info!(
        claimed_jobs = jobs.len(),
        queue_pending_ready = depth_before.pending_ready,
        queue_pending_total = depth_before.pending_total,
        queue_running_total = depth_before.running_total,
        queue_dead_letter_total = depth_before.dead_letter_total,
        "monitoring sync worker claimed job batch"
    );

    let mut join_set = JoinSet::new();
    let mut success_count = 0_usize;
    let mut failed_count = 0_usize;
    let mut panicked_count = 0_usize;

    for job in jobs {
        while join_set.len() >= max_parallel {
            collect_job_outcome(
                join_set.join_next().await,
                &mut success_count,
                &mut failed_count,
                &mut panicked_count,
            );
        }

        let state_cloned = state.clone();
        join_set.spawn(async move { process_claimed_job(state_cloned, job).await });
    }

    while !join_set.is_empty() {
        collect_job_outcome(
            join_set.join_next().await,
            &mut success_count,
            &mut failed_count,
            &mut panicked_count,
        );
    }

    let elapsed = batch_started.elapsed();
    let processed_total = success_count + failed_count + panicked_count;
    let throughput = if elapsed.as_secs_f64() <= f64::EPSILON {
        processed_total as f64
    } else {
        processed_total as f64 / elapsed.as_secs_f64()
    };
    let depth_after = load_queue_depth(&state.db).await.unwrap_or_default();
    info!(
        processed_total,
        success_count,
        failed_count,
        panicked_count,
        elapsed_ms = elapsed.as_millis(),
        throughput_jobs_per_second = format!("{throughput:.2}"),
        queue_pending_ready = depth_after.pending_ready,
        queue_pending_total = depth_after.pending_total,
        queue_running_total = depth_after.running_total,
        queue_dead_letter_total = depth_after.dead_letter_total,
        "monitoring sync worker finished job batch"
    );

    Ok(())
}

async fn process_claimed_job(state: AppState, job: SyncJobRecord) -> AppResult<bool> {
    let outcome = process_job(&state, &job).await;
    match outcome {
        Ok(result) => {
            mark_job_success(&state, &job, job.attempt, &result).await?;
            Ok(true)
        }
        Err(err) => {
            mark_job_failure(&state, &job, job.attempt, &err.to_string()).await?;
            Ok(false)
        }
    }
}

fn collect_job_outcome(
    outcome: Option<Result<AppResult<bool>, tokio::task::JoinError>>,
    success_count: &mut usize,
    failed_count: &mut usize,
    panicked_count: &mut usize,
) {
    match outcome {
        Some(Ok(Ok(true))) => *success_count += 1,
        Some(Ok(Ok(false))) => *failed_count += 1,
        Some(Ok(Err(err))) => {
            warn!(error = %err, "monitoring sync worker failed to finalize claimed job");
            *failed_count += 1;
        }
        Some(Err(err)) => {
            warn!(error = %err, "monitoring sync worker task join error");
            *panicked_count += 1;
        }
        None => {}
    }
}

async fn load_queue_depth(db: &sqlx::PgPool) -> AppResult<QueueDepthRow> {
    sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_total,
            COALESCE(SUM(CASE WHEN status = 'pending' AND run_after <= NOW() THEN 1 ELSE 0 END), 0) AS pending_ready,
            COALESCE(SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END), 0) AS running_total,
            COALESCE(SUM(CASE WHEN status = 'dead_letter' THEN 1 ELSE 0 END), 0) AS dead_letter_total
         FROM cmdb_monitoring_sync_jobs",
    )
    .fetch_one(db)
    .await
    .map_err(AppError::from)
}

async fn process_job(state: &AppState, job: &SyncJobRecord) -> AppResult<SyncSuccessResult> {
    let asset = load_asset(state, job.asset_id).await?;
    let binding_lookup = load_binding_lookup(state, job.asset_id).await?;

    if !is_eligible_asset_class(asset.asset_class.as_str()) {
        return Ok(SyncSuccessResult {
            status: STATUS_SKIPPED.to_string(),
            message: format!(
                "asset class '{}' is not eligible for monitoring auto-provisioning",
                asset.asset_class
            ),
            external_host_id: None,
            source_id: None,
            mapping: json!({}),
        });
    }

    let source = if let Some(source_id) = binding_lookup.as_ref().and_then(|item| item.source_id) {
        match load_monitoring_source_by_id(state, source_id).await {
            Ok(source) => source,
            Err(_) => {
                load_best_monitoring_source(
                    state,
                    asset.site.as_deref(),
                    asset.department.as_deref(),
                )
                .await?
            }
        }
    } else {
        load_best_monitoring_source(state, asset.site.as_deref(), asset.department.as_deref())
            .await?
    };
    let session = build_zabbix_session(state, &source).await?;

    let mapping = resolve_mapping(&asset, &source)?;
    let group_id = ensure_host_group(&session, &mapping.host_group).await?;
    let template_ids = resolve_template_ids(&session, &mapping.templates).await?;
    let proxy_id = resolve_proxy_id(&session, mapping.proxy_name.as_deref()).await?;
    let host = upsert_host(
        &session,
        &asset,
        mapping.host_key.as_str(),
        binding_lookup
            .as_ref()
            .and_then(|item| item.external_host_id.as_deref()),
        group_id.as_str(),
        template_ids.as_slice(),
        proxy_id.as_deref(),
    )
    .await?;

    Ok(SyncSuccessResult {
        status: STATUS_SUCCESS.to_string(),
        message: format!(
            "zabbix host upserted successfully (hostid={}, host={})",
            host.host_id, host.host_key
        ),
        external_host_id: Some(host.host_id),
        source_id: Some(source.id),
        mapping: json!({
            "host_key": mapping.host_key,
            "host_group": mapping.host_group,
            "templates": mapping.templates,
            "proxy_name": mapping.proxy_name
        }),
    })
}

async fn mark_job_success(
    state: &AppState,
    job: &SyncJobRecord,
    attempt: i32,
    result: &SyncSuccessResult,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE cmdb_monitoring_sync_jobs
         SET status = $2,
             attempt = $3,
             completed_at = NOW(),
             last_error = NULL,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(job.id)
    .bind(result.status.as_str())
    .bind(attempt)
    .execute(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO cmdb_monitoring_bindings
            (asset_id, source_system, source_id, external_host_id, last_sync_status, last_sync_message, last_sync_at, mapping)
         VALUES ($1, 'zabbix', $2, $3, $4, $5, NOW(), $6)
         ON CONFLICT (asset_id) DO UPDATE
         SET source_id = EXCLUDED.source_id,
             external_host_id = COALESCE(EXCLUDED.external_host_id, cmdb_monitoring_bindings.external_host_id),
             last_sync_status = EXCLUDED.last_sync_status,
             last_sync_message = EXCLUDED.last_sync_message,
             last_sync_at = NOW(),
             mapping = EXCLUDED.mapping,
             updated_at = NOW()",
    )
    .bind(job.asset_id)
    .bind(result.source_id)
    .bind(result.external_host_id.clone())
    .bind(result.status.as_str())
    .bind(truncate_message(result.message.as_str()))
    .bind(result.mapping.clone())
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: WORKER_ACTOR.to_string(),
            action: "cmdb.monitoring_sync.provision".to_string(),
            target_type: "asset".to_string(),
            target_id: Some(job.asset_id.to_string()),
            result: result.status.clone(),
            message: Some(result.message.clone()),
            metadata: json!({
                "job_id": job.id,
                "attempt": attempt,
                "trigger_source": job.trigger_source,
                "external_host_id": result.external_host_id
            }),
        },
    )
    .await;

    Ok(())
}

async fn mark_job_failure(
    state: &AppState,
    job: &SyncJobRecord,
    attempt: i32,
    error_message: &str,
) -> AppResult<()> {
    let truncated_error = truncate_message(error_message);
    let max_attempts = if job.max_attempts > 0 {
        job.max_attempts
    } else {
        1
    };
    let should_dead_letter = attempt >= max_attempts;

    if should_dead_letter {
        sqlx::query(
            "UPDATE cmdb_monitoring_sync_jobs
             SET status = $2,
                 attempt = $3,
                 completed_at = NOW(),
                 last_error = $4,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(job.id)
        .bind(STATUS_DEAD_LETTER)
        .bind(attempt)
        .bind(truncated_error.clone())
        .execute(&state.db)
        .await?;
    } else {
        let backoff_seconds = retry_backoff_seconds(attempt);
        sqlx::query(
            "UPDATE cmdb_monitoring_sync_jobs
             SET status = $2,
                 attempt = $3,
                 run_after = NOW() + ($4 || ' seconds')::INTERVAL,
                 last_error = $5,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(job.id)
        .bind(STATUS_PENDING)
        .bind(attempt)
        .bind(backoff_seconds)
        .bind(truncated_error.clone())
        .execute(&state.db)
        .await?;
    }

    let binding_status = if should_dead_letter {
        STATUS_DEAD_LETTER
    } else {
        STATUS_FAILED
    };
    sqlx::query(
        "INSERT INTO cmdb_monitoring_bindings
            (asset_id, source_system, last_sync_status, last_sync_message, last_sync_at, mapping)
         VALUES ($1, 'zabbix', $2, $3, NOW(), '{}'::jsonb)
         ON CONFLICT (asset_id) DO UPDATE
         SET last_sync_status = EXCLUDED.last_sync_status,
             last_sync_message = EXCLUDED.last_sync_message,
             last_sync_at = NOW(),
             updated_at = NOW()",
    )
    .bind(job.asset_id)
    .bind(binding_status)
    .bind(truncated_error.clone())
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: WORKER_ACTOR.to_string(),
            action: "cmdb.monitoring_sync.provision".to_string(),
            target_type: "asset".to_string(),
            target_id: Some(job.asset_id.to_string()),
            result: binding_status.to_string(),
            message: Some(truncated_error.clone()),
            metadata: json!({
                "job_id": job.id,
                "attempt": attempt,
                "max_attempts": max_attempts,
                "trigger_source": job.trigger_source
            }),
        },
    )
    .await;

    Ok(())
}

#[derive(Debug, Clone, FromRow)]
struct SyncJobRecord {
    id: i64,
    asset_id: i64,
    trigger_source: String,
    attempt: i32,
    max_attempts: i32,
}

#[derive(Debug, Default, FromRow)]
struct QueueDepthRow {
    pending_total: i64,
    pending_ready: i64,
    running_total: i64,
    dead_letter_total: i64,
}

#[derive(Debug, FromRow)]
struct AssetRecord {
    id: i64,
    asset_class: String,
    name: String,
    hostname: Option<String>,
    ip: Option<String>,
    site: Option<String>,
    department: Option<String>,
    custom_fields: Value,
}

#[derive(Debug, FromRow)]
struct MonitoringBindingLookup {
    source_id: Option<i64>,
    external_host_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct MonitoringSourceRecord {
    id: i64,
    endpoint: String,
    proxy_endpoint: Option<String>,
    auth_type: String,
    username: Option<String>,
    secret_ref: String,
    secret_ciphertext: Option<String>,
}

#[derive(Debug, Clone)]
struct ZabbixMapping {
    host_key: String,
    host_group: String,
    templates: Vec<String>,
    proxy_name: Option<String>,
}

#[derive(Debug)]
struct SyncSuccessResult {
    status: String,
    message: String,
    external_host_id: Option<String>,
    source_id: Option<i64>,
    mapping: Value,
}

#[derive(Debug)]
struct UpsertedHost {
    host_id: String,
    host_key: String,
}

#[derive(Debug)]
struct ZabbixSession {
    client: reqwest::Client,
    endpoint: String,
    auth_token: Option<String>,
    bearer_token: Option<String>,
}

async fn load_asset(state: &AppState, asset_id: i64) -> AppResult<AssetRecord> {
    let item: Option<AssetRecord> = sqlx::query_as(
        "SELECT id, asset_class, name, hostname, ip, site, department, custom_fields
         FROM assets
         WHERE id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;

    item.ok_or_else(|| AppError::NotFound(format!("asset {asset_id} not found")))
}

async fn load_binding_lookup(
    state: &AppState,
    asset_id: i64,
) -> AppResult<Option<MonitoringBindingLookup>> {
    let item: Option<MonitoringBindingLookup> = sqlx::query_as(
        "SELECT source_id, external_host_id
         FROM cmdb_monitoring_bindings
         WHERE asset_id = $1",
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(item)
}

async fn load_best_monitoring_source(
    state: &AppState,
    site: Option<&str>,
    department: Option<&str>,
) -> AppResult<MonitoringSourceRecord> {
    let item: Option<MonitoringSourceRecord> = sqlx::query_as(
        "SELECT id, endpoint, proxy_endpoint, auth_type, username, secret_ref, secret_ciphertext
         FROM monitoring_sources
         WHERE is_enabled = TRUE
           AND source_type = 'zabbix'
           AND (site IS NULL OR site = $1)
           AND (department IS NULL OR department = $2)
         ORDER BY
            (CASE WHEN site = $1 THEN 2 WHEN site IS NULL THEN 1 ELSE 0 END +
             CASE WHEN department = $2 THEN 2 WHEN department IS NULL THEN 1 ELSE 0 END) DESC,
            id DESC
         LIMIT 1",
    )
    .bind(site.map(|v| v.to_string()))
    .bind(department.map(|v| v.to_string()))
    .fetch_optional(&state.db)
    .await?;

    item.ok_or_else(|| {
        AppError::Validation(
            "no enabled Zabbix monitoring source matched asset site/department".to_string(),
        )
    })
}

async fn load_monitoring_source_by_id(
    state: &AppState,
    source_id: i64,
) -> AppResult<MonitoringSourceRecord> {
    let item: Option<MonitoringSourceRecord> = sqlx::query_as(
        "SELECT id, endpoint, proxy_endpoint, auth_type, username, secret_ref, secret_ciphertext
         FROM monitoring_sources
         WHERE id = $1
           AND is_enabled = TRUE
           AND source_type = 'zabbix'",
    )
    .bind(source_id)
    .fetch_optional(&state.db)
    .await?;
    item.ok_or_else(|| {
        AppError::Validation(format!(
            "zabbix monitoring source {} is disabled or missing",
            source_id
        ))
    })
}

async fn build_zabbix_session(
    state: &AppState,
    source: &MonitoringSourceRecord,
) -> AppResult<ZabbixSession> {
    let endpoint = normalize_zabbix_endpoint(source.endpoint.as_str())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| AppError::Internal(anyhow::anyhow!(err)))?;

    match source.auth_type.as_str() {
        "token" => {
            let secret = resolve_monitoring_secret(
                source.secret_ref.as_str(),
                source.secret_ciphertext.as_deref(),
                state.monitoring_secret.encryption_key.as_deref(),
            )?;
            Ok(ZabbixSession {
                client,
                endpoint,
                auth_token: None,
                bearer_token: Some(secret),
            })
        }
        "basic" => {
            let username = source.username.clone().ok_or_else(|| {
                AppError::Validation(
                    "monitoring source requires username for basic auth".to_string(),
                )
            })?;
            let password = resolve_monitoring_secret(
                source.secret_ref.as_str(),
                source.secret_ciphertext.as_deref(),
                state.monitoring_secret.encryption_key.as_deref(),
            )?;
            let params = json!({
                "username": username,
                "password": password
            });
            let result =
                rpc_call_raw(&client, endpoint.as_str(), None, None, "user.login", params).await?;
            let auth_token = result.as_str().map(ToString::to_string).ok_or_else(|| {
                AppError::Validation("zabbix user.login returned non-string token".to_string())
            })?;
            Ok(ZabbixSession {
                client,
                endpoint,
                auth_token: Some(auth_token),
                bearer_token: None,
            })
        }
        _ => Err(AppError::Validation(format!(
            "unsupported monitoring auth_type '{}'",
            source.auth_type
        ))),
    }
}

fn resolve_mapping(
    asset: &AssetRecord,
    source: &MonitoringSourceRecord,
) -> AppResult<ZabbixMapping> {
    let custom = asset.custom_fields.as_object().cloned().unwrap_or_default();
    let host_key = if let Some(hostname) = trim_optional(asset.hostname.clone()) {
        sanitize_host_key(hostname.as_str())
    } else {
        sanitize_host_key(format!("asset-{}", asset.id).as_str())
    };

    let host_group = custom
        .get("monitoring_host_group")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_host_group);

    let templates = resolve_template_names(asset, &custom);

    let proxy_name = custom
        .get("monitoring_proxy")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if source.proxy_endpoint.is_some() {
                default_proxy_name()
            } else {
                None
            }
        });

    Ok(ZabbixMapping {
        host_key,
        host_group,
        templates,
        proxy_name,
    })
}

fn resolve_template_names(asset: &AssetRecord, custom: &Map<String, Value>) -> Vec<String> {
    if let Some(raw) = custom.get("monitoring_templates") {
        if let Some(items) = raw.as_array() {
            let templates = items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if !templates.is_empty() {
                return templates;
            }
        }
    }

    if let Some(value) = custom
        .get("monitoring_template")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        return vec![value.to_string()];
    }

    match asset.asset_class.trim().to_ascii_lowercase().as_str() {
        "server" | "virtual_machine" | "vm" | "database" => default_server_template()
            .map(|item| vec![item])
            .unwrap_or_default(),
        "network_device" => default_network_template()
            .map(|item| vec![item])
            .unwrap_or_default(),
        "container" => default_container_template()
            .map(|item| vec![item])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

async fn ensure_host_group(session: &ZabbixSession, name: &str) -> AppResult<String> {
    let result = rpc_call(
        session,
        "hostgroup.get",
        json!({
            "output": ["groupid", "name"],
            "filter": { "name": [name] }
        }),
    )
    .await?;
    if let Some(group_id) = result
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("groupid"))
        .and_then(Value::as_str)
    {
        return Ok(group_id.to_string());
    }

    let created = rpc_call(
        session,
        "hostgroup.create",
        json!({
            "name": name
        }),
    )
    .await?;

    let group_id = created
        .get("groupids")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AppError::Validation("zabbix hostgroup.create returned no groupids".to_string())
        })?;

    Ok(group_id.to_string())
}

async fn resolve_template_ids(
    session: &ZabbixSession,
    templates: &[String],
) -> AppResult<Vec<String>> {
    let mut ids = Vec::new();
    for template in templates {
        let result = rpc_call(
            session,
            "template.get",
            json!({
                "output": ["templateid", "host"],
                "filter": { "host": [template] }
            }),
        )
        .await?;

        let Some(template_id) = result
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("templateid"))
            .and_then(Value::as_str)
        else {
            return Err(AppError::Validation(format!(
                "zabbix template '{}' not found",
                template
            )));
        };
        ids.push(template_id.to_string());
    }
    Ok(ids)
}

async fn resolve_proxy_id(
    session: &ZabbixSession,
    proxy_name: Option<&str>,
) -> AppResult<Option<String>> {
    let Some(proxy_name) = proxy_name else {
        return Ok(None);
    };
    let result = rpc_call(
        session,
        "proxy.get",
        json!({
            "output": ["proxyid", "host"],
            "filter": { "host": [proxy_name] }
        }),
    )
    .await?;

    let Some(proxy_id) = result
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("proxyid"))
        .and_then(Value::as_str)
    else {
        return Err(AppError::Validation(format!(
            "zabbix proxy '{}' not found",
            proxy_name
        )));
    };

    Ok(Some(proxy_id.to_string()))
}

async fn upsert_host(
    session: &ZabbixSession,
    asset: &AssetRecord,
    host_key: &str,
    known_host_id: Option<&str>,
    host_group_id: &str,
    template_ids: &[String],
    proxy_id: Option<&str>,
) -> AppResult<UpsertedHost> {
    let mut existing_host_id = known_host_id.map(ToString::to_string);
    if existing_host_id.is_some() {
        let exists = lookup_host_by_id(
            session,
            existing_host_id
                .as_deref()
                .expect("checked known_host_id exists"),
        )
        .await?;
        if !exists {
            existing_host_id = None;
        }
    }
    if existing_host_id.is_none() {
        if let Some(existing) = lookup_host_by_key(session, host_key).await? {
            existing_host_id = Some(existing);
        }
    }

    if existing_host_id.is_none() {
        let interfaces = build_host_interfaces(asset)?;
        let mut create_payload = json!({
            "host": host_key,
            "name": asset.name,
            "groups": [
                { "groupid": host_group_id }
            ],
            "interfaces": interfaces
        });
        if !template_ids.is_empty() {
            create_payload
                .as_object_mut()
                .expect("create payload object")
                .insert(
                    "templates".to_string(),
                    Value::Array(
                        template_ids
                            .iter()
                            .map(|id| json!({ "templateid": id }))
                            .collect(),
                    ),
                );
        }
        if let Some(proxy_id) = proxy_id {
            create_payload
                .as_object_mut()
                .expect("create payload object")
                .insert(
                    "proxy_hostid".to_string(),
                    Value::String(proxy_id.to_string()),
                );
        }

        let created = rpc_call(session, "host.create", create_payload).await?;
        let host_id = created
            .get("hostids")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AppError::Validation("zabbix host.create returned no hostids".to_string())
            })?;

        return Ok(UpsertedHost {
            host_id: host_id.to_string(),
            host_key: host_key.to_string(),
        });
    }

    let host_id = existing_host_id.expect("checked is_some");
    let mut update_payload = json!({
        "hostid": host_id,
        "name": asset.name,
        "groups": [
            { "groupid": host_group_id }
        ]
    });
    if !template_ids.is_empty() {
        update_payload
            .as_object_mut()
            .expect("update payload object")
            .insert(
                "templates".to_string(),
                Value::Array(
                    template_ids
                        .iter()
                        .map(|id| json!({ "templateid": id }))
                        .collect(),
                ),
            );
    }
    if let Some(proxy_id) = proxy_id {
        update_payload
            .as_object_mut()
            .expect("update payload object")
            .insert(
                "proxy_hostid".to_string(),
                Value::String(proxy_id.to_string()),
            );
    }
    rpc_call(session, "host.update", update_payload).await?;

    Ok(UpsertedHost {
        host_id,
        host_key: host_key.to_string(),
    })
}

async fn lookup_host_by_key(session: &ZabbixSession, host_key: &str) -> AppResult<Option<String>> {
    let result = rpc_call(
        session,
        "host.get",
        json!({
            "output": ["hostid", "host"],
            "filter": { "host": [host_key] }
        }),
    )
    .await?;

    let host_id = result
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("hostid"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(host_id)
}

async fn lookup_host_by_id(session: &ZabbixSession, host_id: &str) -> AppResult<bool> {
    let result = rpc_call(
        session,
        "host.get",
        json!({
            "output": ["hostid"],
            "hostids": [host_id]
        }),
    )
    .await?;
    Ok(result
        .as_array()
        .map(|items| !items.is_empty())
        .unwrap_or(false))
}

fn build_host_interfaces(asset: &AssetRecord) -> AppResult<Value> {
    let class = asset.asset_class.trim().to_ascii_lowercase();
    let (iface_type, port) = match class.as_str() {
        "network_device" => (2, "161"),
        _ => (1, "10050"),
    };

    if let Some(ip) = trim_optional(asset.ip.clone()) {
        return Ok(json!([{
            "type": iface_type,
            "main": 1,
            "useip": 1,
            "ip": ip,
            "dns": "",
            "port": port
        }]));
    }
    if let Some(hostname) = trim_optional(asset.hostname.clone()) {
        return Ok(json!([{
            "type": iface_type,
            "main": 1,
            "useip": 0,
            "ip": "0.0.0.0",
            "dns": hostname,
            "port": port
        }]));
    }

    Err(AppError::Validation(
        "asset must have ip or hostname for zabbix provisioning".to_string(),
    ))
}

async fn rpc_call(session: &ZabbixSession, method: &str, params: Value) -> AppResult<Value> {
    rpc_call_raw(
        &session.client,
        session.endpoint.as_str(),
        session.auth_token.as_deref(),
        session.bearer_token.as_deref(),
        method,
        params,
    )
    .await
}

async fn rpc_call_raw(
    client: &reqwest::Client,
    endpoint: &str,
    auth_token: Option<&str>,
    bearer_token: Option<&str>,
    method: &str,
    params: Value,
) -> AppResult<Value> {
    let mut body = Map::new();
    body.insert("jsonrpc".to_string(), Value::String("2.0".to_string()));
    body.insert("method".to_string(), Value::String(method.to_string()));
    body.insert("params".to_string(), params);
    body.insert("id".to_string(), Value::Number(1.into()));
    if let Some(auth_token) = auth_token {
        body.insert("auth".to_string(), Value::String(auth_token.to_string()));
    }

    let mut headers = HeaderMap::new();
    if let Some(bearer) = bearer_token {
        let value = format!("Bearer {bearer}");
        let header = HeaderValue::from_str(value.as_str())
            .map_err(|_| AppError::Validation("invalid bearer token header value".to_string()))?;
        headers.insert(AUTHORIZATION, header);
    }

    let response = client
        .post(endpoint)
        .headers(headers)
        .json(&Value::Object(body))
        .send()
        .await
        .map_err(|err| AppError::Validation(format!("zabbix request failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Validation(format!(
            "zabbix returned HTTP {}: {}",
            status,
            truncate_message(body.as_str())
        )));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|err| AppError::Validation(format!("zabbix response decode failed: {err}")))?;
    if let Some(error) = payload.get("error") {
        return Err(AppError::Validation(format!(
            "zabbix api error: {}",
            truncate_message(error.to_string().as_str())
        )));
    }

    payload
        .get("result")
        .cloned()
        .ok_or_else(|| AppError::Validation("zabbix response missing result field".to_string()))
}

fn normalize_zabbix_endpoint(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "monitoring source endpoint is empty".to_string(),
        ));
    }

    let parsed = reqwest::Url::parse(trimmed).map_err(|_| {
        AppError::Validation("monitoring source endpoint must be a valid URL".to_string())
    })?;

    let mut endpoint = parsed;
    if endpoint.path() == "/" || endpoint.path().is_empty() {
        endpoint.set_path("/api_jsonrpc.php");
    }

    Ok(endpoint.to_string())
}

fn sanitize_host_key(value: &str) -> String {
    let mut key = value.trim().to_ascii_lowercase();
    if key.is_empty() {
        return "asset-unknown".to_string();
    }
    key = key
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    while key.contains("--") {
        key = key.replace("--", "-");
    }
    key.trim_matches('-').to_string()
}

fn retry_backoff_seconds(attempt: i32) -> i32 {
    let attempt = attempt.max(1);
    let exponent = (attempt - 1).min(8) as u32;
    let base = 15_i32.saturating_mul(2_i32.saturating_pow(exponent));
    base.min(900)
}

fn truncate_message(message: &str) -> String {
    const MAX: usize = 1024;
    if message.len() <= MAX {
        message.to_string()
    } else {
        format!("{}...", &message[..MAX])
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

fn worker_enabled() -> bool {
    env::var("MONITORING_SYNC_WORKER_ENABLED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

fn poll_interval_seconds() -> u64 {
    env::var("MONITORING_SYNC_POLL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.clamp(1, 60))
        .unwrap_or(3)
}

fn worker_batch_size() -> i64 {
    env::var("MONITORING_SYNC_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .map(|value| value.clamp(1, 200))
        .unwrap_or(20)
}

fn worker_max_parallel() -> usize {
    env::var("MONITORING_SYNC_MAX_PARALLEL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(1, 16))
        .unwrap_or(4)
}

fn default_proxy_name() -> Option<String> {
    env::var("MONITORING_DEFAULT_ZABBIX_PROXY")
        .ok()
        .and_then(|value| trim_optional(Some(value)))
}

fn default_host_group() -> String {
    env::var("MONITORING_DEFAULT_ZABBIX_HOST_GROUP")
        .ok()
        .and_then(|value| trim_optional(Some(value)))
        .unwrap_or_else(|| "CloudOps Hosts".to_string())
}

fn default_server_template() -> Option<String> {
    env::var("MONITORING_DEFAULT_ZABBIX_TEMPLATE_SERVER")
        .ok()
        .and_then(|value| trim_optional(Some(value)))
        .or_else(|| Some("Linux by Zabbix agent".to_string()))
}

fn default_network_template() -> Option<String> {
    env::var("MONITORING_DEFAULT_ZABBIX_TEMPLATE_NETWORK")
        .ok()
        .and_then(|value| trim_optional(Some(value)))
}

fn default_container_template() -> Option<String> {
    env::var("MONITORING_DEFAULT_ZABBIX_TEMPLATE_CONTAINER")
        .ok()
        .and_then(|value| trim_optional(Some(value)))
}

#[cfg(test)]
mod tests {
    use super::{retry_backoff_seconds, sanitize_host_key};

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(retry_backoff_seconds(1), 15);
        assert_eq!(retry_backoff_seconds(2), 30);
        assert_eq!(retry_backoff_seconds(3), 60);
        assert_eq!(retry_backoff_seconds(9), 900);
        assert_eq!(retry_backoff_seconds(20), 900);
    }

    #[test]
    fn host_key_normalization() {
        assert_eq!(sanitize_host_key("APP Server#01"), "app-server-01");
        assert_eq!(sanitize_host_key("  prod.db.local  "), "prod.db.local");
    }
}
