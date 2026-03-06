use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/weekly-digest", get(get_weekly_digest))
        .route("/cockpit/weekly-digest/export", get(export_weekly_digest))
}

#[derive(Debug, Deserialize, Default)]
struct WeeklyDigestQuery {
    week_start: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct WeeklyDigestExportQuery {
    week_start: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct WeeklyDigestMetrics {
    open_critical_alerts: i64,
    open_warning_alerts: i64,
    suppressed_alert_threads: i64,
    stale_open_tickets: i64,
    workflow_approval_backlog: i64,
    playbook_approval_backlog: i64,
    backup_failed_policies: i64,
    drill_failed_policies: i64,
    locked_local_accounts: i64,
    local_accounts_without_mfa: i64,
}

#[derive(Debug, Serialize, Clone)]
struct WeeklyDigestResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    week_start: String,
    week_end: String,
    metrics: WeeklyDigestMetrics,
    top_risks: Vec<String>,
    unresolved_items: Vec<String>,
    recommended_actions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WeeklyDigestExportResponse {
    generated_at: DateTime<Utc>,
    digest_key: String,
    format: String,
    content: String,
}

async fn get_weekly_digest(
    State(state): State<AppState>,
    Query(query): Query<WeeklyDigestQuery>,
) -> AppResult<Json<WeeklyDigestResponse>> {
    let digest = build_weekly_digest(&state, query.week_start).await?;
    Ok(Json(digest))
}

async fn export_weekly_digest(
    State(state): State<AppState>,
    Query(query): Query<WeeklyDigestExportQuery>,
) -> AppResult<Json<WeeklyDigestExportResponse>> {
    let digest = build_weekly_digest(&state, query.week_start).await?;
    let format = query
        .format
        .unwrap_or_else(|| "csv".to_string())
        .trim()
        .to_ascii_lowercase();

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&digest)
            .map_err(|err| AppError::Validation(format!("failed to serialize digest json: {err}")))?,
        "csv" => digest_to_csv(&digest),
        _ => {
            return Err(AppError::Validation(
                "format must be one of: csv, json".to_string(),
            ));
        }
    };

    Ok(Json(WeeklyDigestExportResponse {
        generated_at: digest.generated_at,
        digest_key: digest.digest_key,
        format,
        content,
    }))
}

async fn build_weekly_digest(
    state: &AppState,
    week_start_raw: Option<String>,
) -> AppResult<WeeklyDigestResponse> {
    let week_start = parse_week_start(week_start_raw)?;
    let week_end = week_start + Duration::days(6);
    let window_end_exclusive = week_end + Duration::days(1);

    let open_critical_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM unified_alerts
         WHERE status IN ('open', 'acknowledged')
           AND severity = 'critical'",
    )
    .fetch_one(&state.db)
    .await?;

    let open_warning_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM unified_alerts
         WHERE status IN ('open', 'acknowledged')
           AND severity = 'warning'",
    )
    .fetch_one(&state.db)
    .await?;

    let suppressed_alert_threads: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT alert_id)
         FROM alert_policy_actions
         WHERE action = 'suppressed'
           AND created_at >= $1
           AND created_at < $2",
    )
    .bind(week_start)
    .bind(window_end_exclusive)
    .fetch_one(&state.db)
    .await?;

    let stale_open_tickets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM tickets
         WHERE status IN ('open', 'in_progress')
           AND created_at < NOW() - INTERVAL '24 hour'",
    )
    .fetch_one(&state.db)
    .await?;

    let workflow_approval_backlog: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_requests
         WHERE status = 'pending_approval'",
    )
    .fetch_one(&state.db)
    .await?;

    let playbook_approval_backlog: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM workflow_playbook_approval_requests
         WHERE status = 'pending'
           AND expires_at > NOW()",
    )
    .fetch_one(&state.db)
    .await?;

    let backup_failed_policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policies
         WHERE last_backup_status = 'failed'",
    )
    .fetch_one(&state.db)
    .await?;

    let drill_failed_policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM ops_backup_policies
         WHERE drill_enabled = TRUE
           AND last_drill_status = 'failed'",
    )
    .fetch_one(&state.db)
    .await?;

    let locked_local_accounts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM auth_local_credentials
         WHERE locked_until IS NOT NULL
           AND locked_until > NOW()",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let local_accounts_without_mfa: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM auth_local_credentials
         WHERE mfa_enabled = FALSE",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let metrics = WeeklyDigestMetrics {
        open_critical_alerts,
        open_warning_alerts,
        suppressed_alert_threads,
        stale_open_tickets,
        workflow_approval_backlog,
        playbook_approval_backlog,
        backup_failed_policies,
        drill_failed_policies,
        locked_local_accounts,
        local_accounts_without_mfa,
    };

    let mut top_risks = Vec::new();
    if open_critical_alerts > 0 {
        top_risks.push(format!("{open_critical_alerts} critical alerts remain open/acknowledged."));
    }
    if backup_failed_policies > 0 {
        top_risks.push(format!("{backup_failed_policies} backup policies report latest failure."));
    }
    if drill_failed_policies > 0 {
        top_risks.push(format!("{drill_failed_policies} drill policies report latest failure."));
    }
    if locked_local_accounts > 0 {
        top_risks.push(format!("{locked_local_accounts} local accounts are currently locked."));
    }
    if top_risks.is_empty() {
        top_risks.push("No critical blocker detected in weekly digest snapshot.".to_string());
    }

    let mut unresolved_items = Vec::new();
    if stale_open_tickets > 0 {
        unresolved_items.push(format!(
            "{stale_open_tickets} open tickets have exceeded 24h without closure."
        ));
    }
    if workflow_approval_backlog > 0 || playbook_approval_backlog > 0 {
        unresolved_items.push(format!(
            "Approval backlog: workflow={}, playbook={}",
            workflow_approval_backlog, playbook_approval_backlog
        ));
    }
    if suppressed_alert_threads > 0 {
        unresolved_items.push(format!(
            "{suppressed_alert_threads} alert threads were suppressed this week; validate no incident is hidden."
        ));
    }
    if unresolved_items.is_empty() {
        unresolved_items.push("No unresolved item above digest threshold.".to_string());
    }

    let mut recommended_actions = Vec::new();
    if open_critical_alerts > 0 {
        recommended_actions.push("Escalate critical alerts and confirm ownership in ticket queue today.".to_string());
    }
    if backup_failed_policies > 0 || drill_failed_policies > 0 {
        recommended_actions.push(
            "Run backup/drill manually after destination validation and attach remediation evidence.".to_string(),
        );
    }
    if local_accounts_without_mfa > 0 {
        recommended_actions.push(format!(
            "Review {local_accounts_without_mfa} local accounts without MFA and enforce enrollment policy."
        ));
    }
    if workflow_approval_backlog > 0 || playbook_approval_backlog > 0 {
        recommended_actions.push(
            "Clear approval queue to reduce high-risk remediation lead time.".to_string(),
        );
    }
    if recommended_actions.is_empty() {
        recommended_actions.push("Keep current cadence and rerun digest next week for trend comparison.".to_string());
    }

    let digest_key = format!("weekly-{}", week_start.format("%Y-%m-%d"));

    Ok(WeeklyDigestResponse {
        generated_at: Utc::now(),
        digest_key,
        week_start: week_start.format("%Y-%m-%d").to_string(),
        week_end: week_end.format("%Y-%m-%d").to_string(),
        metrics,
        top_risks,
        unresolved_items,
        recommended_actions,
    })
}

fn parse_week_start(value: Option<String>) -> AppResult<DateTime<Utc>> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return default_week_start();
            }
            let date = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
                AppError::Validation("week_start must use YYYY-MM-DD format".to_string())
            })?;
            Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight")))
        }
        None => default_week_start(),
    }
}

fn default_week_start() -> AppResult<DateTime<Utc>> {
    let today = Utc::now().date_naive();
    let weekday = today.weekday().number_from_monday() as i64;
    let monday = today - Duration::days(weekday - 1);
    Ok(Utc.from_utc_datetime(&monday.and_hms_opt(0, 0, 0).expect("midnight")))
}

fn digest_to_csv(digest: &WeeklyDigestResponse) -> String {
    let mut lines = Vec::new();
    lines.push("field,value".to_string());
    lines.push(format!("digest_key,{}", digest.digest_key));
    lines.push(format!("generated_at,{}", digest.generated_at.to_rfc3339()));
    lines.push(format!("week_start,{}", digest.week_start));
    lines.push(format!("week_end,{}", digest.week_end));
    lines.push(format!(
        "open_critical_alerts,{}",
        digest.metrics.open_critical_alerts
    ));
    lines.push(format!(
        "open_warning_alerts,{}",
        digest.metrics.open_warning_alerts
    ));
    lines.push(format!(
        "suppressed_alert_threads,{}",
        digest.metrics.suppressed_alert_threads
    ));
    lines.push(format!("stale_open_tickets,{}", digest.metrics.stale_open_tickets));
    lines.push(format!(
        "workflow_approval_backlog,{}",
        digest.metrics.workflow_approval_backlog
    ));
    lines.push(format!(
        "playbook_approval_backlog,{}",
        digest.metrics.playbook_approval_backlog
    ));
    lines.push(format!(
        "backup_failed_policies,{}",
        digest.metrics.backup_failed_policies
    ));
    lines.push(format!(
        "drill_failed_policies,{}",
        digest.metrics.drill_failed_policies
    ));
    lines.push(format!(
        "locked_local_accounts,{}",
        digest.metrics.locked_local_accounts
    ));
    lines.push(format!(
        "local_accounts_without_mfa,{}",
        digest.metrics.local_accounts_without_mfa
    ));
    lines.push(format!("top_risks,{}", digest.top_risks.join(" | ")));
    lines.push(format!(
        "unresolved_items,{}",
        digest.unresolved_items.join(" | ")
    ));
    lines.push(format!(
        "recommended_actions,{}",
        digest.recommended_actions.join(" | ")
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Utc};

    use super::{default_week_start, digest_to_csv, WeeklyDigestMetrics, WeeklyDigestResponse};

    #[test]
    fn week_start_defaults_to_monday() {
        let monday = default_week_start().expect("week start");
        assert_eq!(monday.weekday().number_from_monday(), 1);
    }

    #[test]
    fn digest_csv_contains_core_fields() {
        let digest = WeeklyDigestResponse {
            generated_at: Utc::now(),
            digest_key: "weekly-2026-03-02".to_string(),
            week_start: "2026-03-02".to_string(),
            week_end: "2026-03-08".to_string(),
            metrics: WeeklyDigestMetrics {
                open_critical_alerts: 1,
                open_warning_alerts: 2,
                suppressed_alert_threads: 3,
                stale_open_tickets: 4,
                workflow_approval_backlog: 5,
                playbook_approval_backlog: 6,
                backup_failed_policies: 1,
                drill_failed_policies: 1,
                locked_local_accounts: 0,
                local_accounts_without_mfa: 2,
            },
            top_risks: vec!["risk".to_string()],
            unresolved_items: vec!["item".to_string()],
            recommended_actions: vec!["action".to_string()],
        };

        let csv = digest_to_csv(&digest);
        assert!(csv.contains("digest_key"));
        assert!(csv.contains("open_critical_alerts,1"));
        assert!(csv.contains("recommended_actions,action"));
    }
}
