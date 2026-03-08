use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sqlx::FromRow;
use tracing::warn;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    error::{AppError, AppResult},
    state::AppState,
};

const AUTH_USER_HEADER: &str = "x-auth-user";

#[derive(Debug, FromRow)]
struct PermissionCheckRecord {
    user_exists: bool,
    allowed: bool,
}

#[derive(Debug, FromRow)]
struct SessionUserRecord {
    username: String,
    auth_source: String,
    last_seen_at: DateTime<Utc>,
}

pub async fn rbac_guard(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> AppResult<Response> {
    if !state.rbac_enabled {
        return Ok(next.run(request).await);
    }

    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let user = resolve_auth_user(&state, request.headers()).await?;
    let permission = match required_permission(&method, &path) {
        Some(value) => value,
        None => {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: user.clone(),
                    action: "auth.permission_denied".to_string(),
                    target_type: "route".to_string(),
                    target_id: Some(path.clone()),
                    result: "denied".to_string(),
                    message: Some(format!(
                        "no RBAC permission mapping found for route '{path}'"
                    )),
                    metadata: json!({
                        "method": method.to_string(),
                        "path": path
                    }),
                },
            )
            .await;

            return Err(AppError::Forbidden(format!(
                "no RBAC permission mapping found for route '{path}'"
            )));
        }
    };

    let permission_check = check_permission(&state, &user, &permission).await?;

    if !permission_check.user_exists {
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: user.clone(),
                action: "auth.login".to_string(),
                target_type: "user".to_string(),
                target_id: Some(user.clone()),
                result: "failed".to_string(),
                message: Some("user does not exist or is disabled".to_string()),
                metadata: json!({
                    "method": method.to_string(),
                    "path": path,
                    "permission": permission
                }),
            },
        )
        .await;
        return Err(AppError::Forbidden(format!(
            "user '{user}' does not exist or is disabled"
        )));
    }

    if !permission_check.allowed {
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: user.clone(),
                action: "auth.permission_denied".to_string(),
                target_type: "route".to_string(),
                target_id: Some(path.clone()),
                result: "denied".to_string(),
                message: Some(format!(
                    "permission denied: '{user}' cannot access '{permission}'"
                )),
                metadata: json!({
                    "method": method.to_string(),
                    "path": path,
                    "permission": permission
                }),
            },
        )
        .await;
        warn!(
            user,
            permission,
            method = %method,
            path,
            "request denied by rbac policy"
        );
        return Err(AppError::Forbidden(format!(
            "permission denied: '{user}' cannot access '{permission}'"
        )));
    }

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: user.clone(),
            action: "auth.login".to_string(),
            target_type: "route".to_string(),
            target_id: Some(path.clone()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "method": method.to_string(),
                "permission": permission
            }),
        },
    )
    .await;

    Ok(next.run(request).await)
}

pub async fn resolve_auth_user(state: &AppState, headers: &HeaderMap) -> AppResult<String> {
    if let Some(user) = read_auth_user_header(headers)? {
        enforce_local_fallback_policy(state, &user).await?;
        return Ok(user);
    }

    if let Some(token) = read_bearer_token(headers)? {
        return resolve_user_from_bearer_token(state, &token).await;
    }

    Err(AppError::Forbidden(format!(
        "{AUTH_USER_HEADER} header or bearer token is required"
    )))
}

async fn enforce_local_fallback_policy(state: &AppState, user: &str) -> AppResult<()> {
    let (allowed, reason) = evaluate_local_fallback_policy(
        state.local_auth.fallback_mode.as_str(),
        &state.local_auth.break_glass_users,
        user,
    );
    let result = if allowed { "allowed" } else { "denied" };
    let mode = state.local_auth.fallback_mode.clone();
    let break_glass_users = state.local_auth.break_glass_users.clone();

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: user.to_string(),
            action: "auth.local_fallback".to_string(),
            target_type: "local_fallback_policy".to_string(),
            target_id: Some(user.to_string()),
            result: result.to_string(),
            message: Some(reason.clone()),
            metadata: json!({
                "mode": mode,
                "break_glass_users": break_glass_users,
            }),
        },
    )
    .await;

    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(reason))
    }
}

fn evaluate_local_fallback_policy(
    mode: &str,
    break_glass_users: &[String],
    user: &str,
) -> (bool, String) {
    let normalized_user = user.trim().to_ascii_lowercase();

    match mode {
        "allow_all" => (
            true,
            "local fallback allowed by policy mode allow_all".to_string(),
        ),
        "break_glass_only" => {
            let allowed = break_glass_users
                .iter()
                .any(|item| item.trim().eq_ignore_ascii_case(&normalized_user));
            if allowed {
                (
                    true,
                    "local fallback allowed by break_glass_only policy".to_string(),
                )
            } else {
                (
                    false,
                    format!("local fallback denied by break_glass_only policy for user '{user}'"),
                )
            }
        }
        "disabled" => (
            false,
            "local fallback is disabled by AUTH_LOCAL_FALLBACK_MODE=disabled".to_string(),
        ),
        _ => (
            false,
            format!("unsupported AUTH_LOCAL_FALLBACK_MODE '{mode}'"),
        ),
    }
}

fn read_auth_user_header(headers: &HeaderMap) -> AppResult<Option<String>> {
    let Some(raw) = headers.get(AUTH_USER_HEADER) else {
        return Ok(None);
    };

    let value = raw
        .to_str()
        .map_err(|_| AppError::Forbidden(format!("{AUTH_USER_HEADER} header is invalid")))?;

    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Forbidden(format!(
            "{AUTH_USER_HEADER} header cannot be empty"
        )));
    }

    Ok(Some(value.to_string()))
}

pub fn read_bearer_token(headers: &HeaderMap) -> AppResult<Option<String>> {
    let Some(raw) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };

    let value = raw
        .to_str()
        .map_err(|_| AppError::Forbidden("authorization header is invalid".to_string()))?;

    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Forbidden(
            "authorization header cannot be empty".to_string(),
        ));
    }

    let lower = value.to_ascii_lowercase();
    if !lower.starts_with("bearer ") {
        return Err(AppError::Forbidden(
            "authorization header must use Bearer token".to_string(),
        ));
    }

    let token = value[7..].trim();
    if token.is_empty() {
        return Err(AppError::Forbidden(
            "bearer token cannot be empty".to_string(),
        ));
    }

    Ok(Some(token.to_string()))
}

async fn resolve_user_from_bearer_token(state: &AppState, token: &str) -> AppResult<String> {
    let session: Option<SessionUserRecord> = sqlx::query_as(
        "SELECT u.username, s.auth_source, s.last_seen_at
         FROM auth_sessions s
         INNER JOIN iam_users u ON u.id = s.user_id
         WHERE s.id = $1
           AND s.revoked_at IS NULL
           AND s.expires_at > NOW()
           AND u.is_enabled = TRUE",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await?;

    let session = session
        .ok_or_else(|| AppError::Forbidden("bearer token is invalid or expired".to_string()))?;

    if session.auth_source == "local"
        && is_local_session_idle_expired(
            session.last_seen_at,
            Utc::now(),
            state.local_auth.session_idle_timeout_minutes,
        )
    {
        sqlx::query(
            "UPDATE auth_sessions
             SET revoked_at = NOW()
             WHERE id = $1
               AND revoked_at IS NULL",
        )
        .bind(token)
        .execute(&state.db)
        .await?;

        return Err(AppError::Forbidden(
            "local session expired by idle timeout".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE auth_sessions
         SET last_seen_at = NOW()
         WHERE id = $1
           AND revoked_at IS NULL",
    )
    .bind(token)
    .execute(&state.db)
    .await?;

    Ok(session.username)
}

fn is_local_session_idle_expired(
    last_seen_at: DateTime<Utc>,
    now: DateTime<Utc>,
    idle_timeout_minutes: u32,
) -> bool {
    if idle_timeout_minutes == 0 {
        return false;
    }
    now > last_seen_at + Duration::minutes(idle_timeout_minutes as i64)
}

fn required_permission(method: &Method, path: &str) -> Option<String> {
    let normalized = normalize_rbac_path(path);

    if matches_scope(&normalized, "/audit/logs")
        || matches_scope(&normalized, "/logs")
        || matches_scope(&normalized, "/iam/users")
        || matches_scope(&normalized, "/iam/roles")
        || matches_scope(&normalized, "/users")
        || matches_scope(&normalized, "/roles")
    {
        return Some("system.admin".to_string());
    }

    if *method == Method::POST && is_setup_profile_preview_path(&normalized) {
        return Some("ops.setup.read".to_string());
    }

    let base = if matches_scope(&normalized, "/cmdb/discovery/notification-channels")
        || matches_scope(&normalized, "/cmdb/discovery/notification-templates")
        || matches_scope(&normalized, "/cmdb/discovery/notification-subscriptions")
        || matches_scope(&normalized, "/discovery/notification-channels")
        || matches_scope(&normalized, "/discovery/notification-templates")
        || matches_scope(&normalized, "/discovery/notification-subscriptions")
    {
        "cmdb.notifications"
    } else if matches_scope(&normalized, "/monitoring/sources")
        || matches_scope(&normalized, "/sources")
    {
        "monitoring.sources"
    } else if matches_scope(&normalized, "/monitoring/overview")
        || matches_scope(&normalized, "/overview")
        || matches_scope(&normalized, "/monitoring/layers")
        || matches_scope(&normalized, "/layers")
        || matches_scope(&normalized, "/monitoring/metrics")
        || matches_scope(&normalized, "/metrics")
    {
        "monitoring.sources"
    } else if matches_scope(&normalized, "/streams/sse")
        || matches_scope(&normalized, "/streams")
        || matches_scope(&normalized, "/sse")
    {
        "monitoring.sources"
    } else if matches_scope(&normalized, "/setup")
        || normalized == "/preflight"
        || normalized == "/checklist"
    {
        "ops.setup"
    } else if matches_scope(&normalized, "/ops/cockpit") || matches_scope(&normalized, "/cockpit") {
        "ops.cockpit"
    } else if matches_scope(&normalized, "/alerts") || is_alert_subroute_path(&normalized) {
        "alerts"
    } else if matches_scope(&normalized, "/cmdb/discovery")
        || matches_scope(&normalized, "/discovery")
    {
        "cmdb.discovery"
    } else if matches_scope(&normalized, "/cmdb/field-definitions")
        || matches_scope(&normalized, "/field-definitions")
    {
        "cmdb.field_definitions"
    } else if matches_scope(&normalized, "/topology/maps")
        || matches_scope(&normalized, "/topology")
        || matches_scope(&normalized, "/maps")
    {
        "cmdb.relations"
    } else if matches_scope(&normalized, "/cmdb/relations")
        || matches_scope(&normalized, "/relations")
    {
        "cmdb.relations"
    } else if matches_scope(&normalized, "/cmdb/monitoring-sync")
        || matches_scope(&normalized, "/monitoring-sync")
    {
        "cmdb.assets"
    } else if matches_scope(&normalized, "/cmdb/assets") || matches_scope(&normalized, "/assets") {
        "cmdb.assets"
    } else if matches_scope(&normalized, "/tickets") {
        "tickets"
    } else if matches_scope(&normalized, "/workflow/playbooks")
        || matches_scope(&normalized, "/workflows/playbooks")
        || matches_scope(&normalized, "/playbooks")
    {
        "workflow.playbooks"
    } else if matches_scope(&normalized, "/workflow/templates")
        || matches_scope(&normalized, "/workflows/templates")
        || matches_scope(&normalized, "/templates")
    {
        "workflow.requests"
    } else if matches_scope(&normalized, "/workflow/requests")
        || matches_scope(&normalized, "/workflows/requests")
        || matches_scope(&normalized, "/requests")
    {
        "workflow.requests"
    } else if matches_scope(&normalized, "/workflow/approvals")
        || matches_scope(&normalized, "/workflows/approvals")
        || matches_scope(&normalized, "/approvals")
    {
        "workflow.approvals"
    } else {
        return None;
    };

    let action = if is_read_method(method) {
        "read"
    } else {
        "write"
    };

    Some(format!("{base}.{action}"))
}

fn normalize_rbac_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("/api/v1") {
        if stripped.is_empty() {
            "/".to_string()
        } else {
            stripped.to_string()
        }
    } else {
        path.to_string()
    }
}

fn matches_scope(path: &str, scope: &str) -> bool {
    if path == scope {
        return true;
    }

    path.strip_prefix(scope)
        .map(|rest| rest.starts_with('/'))
        .unwrap_or(false)
}

fn is_read_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn is_alert_subroute_path(path: &str) -> bool {
    if path == "/" || matches_scope(path, "/bulk") || matches_scope(path, "/policies") {
        return true;
    }

    let first_segment = path.trim_start_matches('/').split('/').next().unwrap_or("");
    first_segment
        .parse::<i64>()
        .ok()
        .is_some_and(|value| value > 0)
}

fn is_setup_profile_preview_path(path: &str) -> bool {
    (path.starts_with("/setup/profiles/") || path.starts_with("/profiles/"))
        && path.ends_with("/preview")
}

async fn check_permission(
    state: &AppState,
    user: &str,
    permission: &str,
) -> AppResult<PermissionCheckRecord> {
    let result: PermissionCheckRecord = sqlx::query_as(
        "SELECT
            EXISTS(
                SELECT 1
                FROM iam_users
                WHERE username = $1
                  AND is_enabled = TRUE
            ) AS user_exists,
            EXISTS(
                SELECT 1
                FROM iam_users u
                INNER JOIN iam_user_roles ur ON ur.user_id = u.id
                INNER JOIN iam_role_permissions rp ON rp.role_id = ur.role_id
                INNER JOIN iam_permissions p ON p.id = rp.permission_id
                WHERE u.username = $1
                  AND u.is_enabled = TRUE
                  AND p.permission_key IN ($2, 'system.admin')
            ) AS allowed",
    )
    .bind(user)
    .bind(permission)
    .fetch_one(&state.db)
    .await?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, Method, header::AUTHORIZATION};
    use chrono::{Duration, TimeZone, Utc};

    use super::{
        evaluate_local_fallback_policy, is_local_session_idle_expired, read_bearer_token,
        required_permission,
    };

    fn assert_permission(method: Method, path: &str, expected: &str) {
        assert_eq!(
            required_permission(&method, path).as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn local_fallback_allow_all_allows_any_user() {
        let (allowed, reason) = evaluate_local_fallback_policy("allow_all", &[], "alice");
        assert!(allowed);
        assert!(reason.contains("allow_all"));
    }

    #[test]
    fn local_fallback_break_glass_only_allows_allowlisted_user() {
        let allowlist = vec!["Admin".to_string(), "ops.emergency".to_string()];
        let (allowed, reason) =
            evaluate_local_fallback_policy("break_glass_only", &allowlist, "admin");
        assert!(allowed);
        assert!(reason.contains("break_glass_only"));
    }

    #[test]
    fn local_fallback_break_glass_only_denies_non_allowlisted_user() {
        let allowlist = vec!["admin".to_string()];
        let (allowed, reason) =
            evaluate_local_fallback_policy("break_glass_only", &allowlist, "viewer");
        assert!(!allowed);
        assert!(reason.contains("denied"));
    }

    #[test]
    fn local_fallback_disabled_denies_all_users() {
        let (allowed, reason) = evaluate_local_fallback_policy("disabled", &[], "admin");
        assert!(!allowed);
        assert!(reason.contains("disabled"));
    }

    #[test]
    fn local_session_idle_timeout_zero_disables_expiration() {
        let now = Utc
            .with_ymd_and_hms(2026, 3, 5, 12, 0, 0)
            .single()
            .expect("valid datetime");
        let last_seen = now - Duration::minutes(120);
        assert!(!is_local_session_idle_expired(last_seen, now, 0));
    }

    #[test]
    fn local_session_idle_timeout_allows_within_window() {
        let now = Utc
            .with_ymd_and_hms(2026, 3, 5, 12, 0, 0)
            .single()
            .expect("valid datetime");
        let last_seen = now - Duration::minutes(14);
        assert!(!is_local_session_idle_expired(last_seen, now, 15));
    }

    #[test]
    fn local_session_idle_timeout_expires_after_window() {
        let now = Utc
            .with_ymd_and_hms(2026, 3, 5, 12, 0, 0)
            .single()
            .expect("valid datetime");
        let last_seen = now - Duration::minutes(16);
        assert!(is_local_session_idle_expired(last_seen, now, 15));
    }

    #[test]
    fn maps_assets_read_permission() {
        assert_permission(Method::GET, "/api/v1/cmdb/assets", "cmdb.assets.read");
    }

    #[test]
    fn maps_assets_write_permission() {
        assert_permission(Method::POST, "/api/v1/cmdb/assets", "cmdb.assets.write");
    }

    #[test]
    fn maps_notification_permissions() {
        assert_permission(
            Method::GET,
            "/api/v1/cmdb/discovery/notification-subscriptions",
            "cmdb.notifications.read",
        );
    }

    #[test]
    fn maps_discovery_write_permission() {
        assert_permission(
            Method::POST,
            "/api/v1/cmdb/discovery/jobs/1/run",
            "cmdb.discovery.write",
        );
    }

    #[test]
    fn maps_monitoring_source_permissions() {
        assert_permission(
            Method::GET,
            "/api/v1/monitoring/sources",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/monitoring/overview",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/monitoring/layers/hardware",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/monitoring/metrics",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/streams/sse",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/streams/metrics",
            "monitoring.sources.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/monitoring/sources",
            "monitoring.sources.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/monitoring/sources/1/probe",
            "monitoring.sources.write",
        );
        assert_permission(Method::GET, "/sources", "monitoring.sources.read");
        assert_permission(Method::GET, "/overview", "monitoring.sources.read");
        assert_permission(Method::GET, "/layers/hardware", "monitoring.sources.read");
        assert_permission(Method::GET, "/metrics", "monitoring.sources.read");
        assert_permission(Method::GET, "/sse", "monitoring.sources.read");
        assert_permission(Method::GET, "/streams/metrics", "monitoring.sources.read");
    }

    #[test]
    fn maps_setup_permissions() {
        assert_permission(Method::GET, "/api/v1/setup/preflight", "ops.setup.read");
        assert_permission(Method::GET, "/api/v1/setup/checklist", "ops.setup.read");
        assert_permission(Method::GET, "/api/v1/setup/templates", "ops.setup.read");
        assert_permission(Method::GET, "/api/v1/setup/profiles", "ops.setup.read");
        assert_permission(
            Method::GET,
            "/api/v1/setup/profiles/history",
            "ops.setup.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/setup/templates/identity-safe-baseline/preview",
            "ops.setup.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/setup/templates/identity-safe-baseline/apply",
            "ops.setup.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/setup/profiles/smb-small-office/preview",
            "ops.setup.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/setup/profiles/smb-small-office/apply",
            "ops.setup.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/setup/profiles/history/1/revert",
            "ops.setup.write",
        );
        assert_permission(Method::GET, "/setup/preflight", "ops.setup.read");
        assert_permission(Method::GET, "/preflight", "ops.setup.read");
        assert_permission(Method::GET, "/checklist", "ops.setup.read");
    }

    #[test]
    fn maps_ops_cockpit_permissions() {
        assert_permission(Method::GET, "/api/v1/ops/cockpit/queue", "ops.cockpit.read");
        assert_permission(
            Method::GET,
            "/api/v1/ops/cockpit/next-actions",
            "ops.cockpit.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/ops/cockpit/checklists",
            "ops.cockpit.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/ops/cockpit/checklists/daily-alert-queue-review/complete",
            "ops.cockpit.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/ops/cockpit/checklists/daily-alert-queue-review/exception",
            "ops.cockpit.write",
        );
        assert_permission(
            Method::GET,
            "/api/v1/ops/cockpit/incidents",
            "ops.cockpit.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/ops/cockpit/incidents/1",
            "ops.cockpit.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/ops/cockpit/incidents/1/command",
            "ops.cockpit.write",
        );
        assert_permission(Method::GET, "/cockpit/queue", "ops.cockpit.read");
        assert_permission(Method::GET, "/cockpit/checklists", "ops.cockpit.read");
    }

    #[test]
    fn maps_alert_permissions() {
        assert_permission(Method::GET, "/api/v1/alerts", "alerts.read");
        assert_permission(Method::GET, "/api/v1/alerts/1", "alerts.read");
        assert_permission(Method::GET, "/api/v1/alerts/1/remediation", "alerts.read");
        assert_permission(Method::GET, "/api/v1/alerts/policies", "alerts.read");
        assert_permission(Method::POST, "/api/v1/alerts/1/ack", "alerts.write");
        assert_permission(Method::POST, "/api/v1/alerts/1/close", "alerts.write");
        assert_permission(Method::POST, "/api/v1/alerts/bulk/ack", "alerts.write");
        assert_permission(Method::PATCH, "/api/v1/alerts/policies/1", "alerts.write");
        assert_permission(Method::GET, "/alerts", "alerts.read");
        assert_permission(Method::GET, "/", "alerts.read");
        assert_permission(Method::GET, "/1", "alerts.read");
        assert_permission(Method::POST, "/1/ack", "alerts.write");
        assert_permission(Method::POST, "/bulk/ack", "alerts.write");
        assert_permission(Method::GET, "/policies", "alerts.read");
    }

    #[test]
    fn maps_iam_permission() {
        assert_permission(Method::GET, "/api/v1/iam/users", "system.admin");
    }

    #[test]
    fn maps_relative_assets_permission() {
        assert_permission(Method::GET, "/assets", "cmdb.assets.read");
    }

    #[test]
    fn maps_relative_iam_permission() {
        assert_permission(Method::GET, "/users", "system.admin");
    }

    #[test]
    fn maps_audit_permission() {
        assert_permission(Method::GET, "/api/v1/audit/logs", "system.admin");
    }

    #[test]
    fn maps_relative_audit_permission() {
        assert_permission(Method::GET, "/logs", "system.admin");
    }

    #[test]
    fn workflow_permission_mapping_ready_for_future_routes() {
        assert_permission(
            Method::GET,
            "/api/v1/workflow/templates",
            "workflow.requests.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/workflow/templates",
            "workflow.requests.write",
        );
        assert_permission(
            Method::GET,
            "/api/v1/workflow/requests",
            "workflow.requests.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/workflow/requests",
            "workflow.requests.write",
        );
        assert_permission(
            Method::GET,
            "/api/v1/workflow/approvals",
            "workflow.approvals.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/workflow/approvals",
            "workflow.approvals.write",
        );
        assert_permission(Method::GET, "/templates", "workflow.requests.read");
        assert_permission(Method::GET, "/requests", "workflow.requests.read");
        assert_permission(
            Method::POST,
            "/approvals/10/approve",
            "workflow.approvals.write",
        );
        assert_permission(
            Method::GET,
            "/api/v1/workflow/playbooks",
            "workflow.playbooks.read",
        );
        assert_permission(
            Method::POST,
            "/api/v1/workflow/playbooks/restart-service-safe/dry-run",
            "workflow.playbooks.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/workflow/playbooks/executions/10/replay",
            "workflow.playbooks.write",
        );
        assert_permission(Method::GET, "/playbooks", "workflow.playbooks.read");
    }

    #[test]
    fn maps_ticket_permission() {
        assert_permission(Method::GET, "/api/v1/tickets", "tickets.read");
        assert_permission(Method::POST, "/api/v1/tickets", "tickets.write");
        assert_permission(Method::PATCH, "/api/v1/tickets/1/status", "tickets.write");
        assert_permission(
            Method::GET,
            "/api/v1/tickets/escalation/policy",
            "tickets.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/tickets/escalation/queue",
            "tickets.read",
        );
        assert_permission(
            Method::PUT,
            "/api/v1/tickets/escalation/policy",
            "tickets.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/tickets/escalation/policy/preview",
            "tickets.write",
        );
        assert_permission(
            Method::POST,
            "/api/v1/tickets/escalation/run",
            "tickets.write",
        );
    }

    #[test]
    fn maps_topology_permission() {
        assert_permission(
            Method::GET,
            "/api/v1/topology/maps/site:dc-a",
            "cmdb.relations.read",
        );
        assert_permission(
            Method::GET,
            "/api/v1/topology/diagnostics/edges/12",
            "cmdb.relations.read",
        );
        assert_permission(Method::GET, "/maps/site:dc-a", "cmdb.relations.read");
    }

    #[test]
    fn permission_matrix_covers_existing_protected_endpoints() {
        let coverage = vec![
            (Method::GET, "/api/v1/cmdb/assets", "cmdb.assets.read"),
            (Method::GET, "/api/v1/cmdb/assets/stats", "cmdb.assets.read"),
            (
                Method::GET,
                "/api/v1/cmdb/assets/by-code/QR-1",
                "cmdb.assets.read",
            ),
            (Method::POST, "/api/v1/cmdb/assets", "cmdb.assets.write"),
            (Method::PATCH, "/api/v1/cmdb/assets/1", "cmdb.assets.write"),
            (
                Method::GET,
                "/api/v1/cmdb/assets/1/graph",
                "cmdb.assets.read",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/assets/1/impact",
                "cmdb.assets.read",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/assets/1/monitoring-binding",
                "cmdb.assets.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/assets/1/monitoring-sync",
                "cmdb.assets.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/monitoring-sync/jobs",
                "cmdb.assets.read",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/field-definitions",
                "cmdb.field_definitions.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/field-definitions",
                "cmdb.field_definitions.write",
            ),
            (
                Method::PATCH,
                "/api/v1/cmdb/field-definitions/1",
                "cmdb.field_definitions.write",
            ),
            (Method::GET, "/api/v1/cmdb/relations", "cmdb.relations.read"),
            (
                Method::GET,
                "/api/v1/topology/maps/global",
                "cmdb.relations.read",
            ),
            (
                Method::GET,
                "/api/v1/topology/diagnostics/edges/1",
                "cmdb.relations.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/relations",
                "cmdb.relations.write",
            ),
            (
                Method::GET,
                "/api/v1/workflow/templates",
                "workflow.requests.read",
            ),
            (
                Method::POST,
                "/api/v1/workflow/requests",
                "workflow.requests.write",
            ),
            (
                Method::POST,
                "/api/v1/workflow/approvals/1/approve",
                "workflow.approvals.write",
            ),
            (
                Method::GET,
                "/api/v1/workflow/playbooks",
                "workflow.playbooks.read",
            ),
            (
                Method::POST,
                "/api/v1/workflow/playbooks/restart-service-safe/dry-run",
                "workflow.playbooks.write",
            ),
            (Method::GET, "/api/v1/ops/cockpit/queue", "ops.cockpit.read"),
            (
                Method::GET,
                "/api/v1/ops/cockpit/next-actions",
                "ops.cockpit.read",
            ),
            (
                Method::GET,
                "/api/v1/ops/cockpit/checklists",
                "ops.cockpit.read",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/checklists/daily-alert-queue-review/complete",
                "ops.cockpit.write",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/checklists/daily-alert-queue-review/exception",
                "ops.cockpit.write",
            ),
            (
                Method::GET,
                "/api/v1/ops/cockpit/change-calendar",
                "ops.cockpit.read",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/change-calendar/conflicts",
                "ops.cockpit.write",
            ),
            (
                Method::GET,
                "/api/v1/ops/cockpit/backup/restore-evidence",
                "ops.cockpit.read",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/backup/runs/1/restore-evidence",
                "ops.cockpit.write",
            ),
            (
                Method::PATCH,
                "/api/v1/ops/cockpit/backup/restore-evidence/1",
                "ops.cockpit.write",
            ),
            (
                Method::GET,
                "/api/v1/ops/cockpit/handover-digest",
                "ops.cockpit.read",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/handover-digest/items/ticket:1/close",
                "ops.cockpit.write",
            ),
            (
                Method::GET,
                "/api/v1/ops/cockpit/incidents",
                "ops.cockpit.read",
            ),
            (
                Method::POST,
                "/api/v1/ops/cockpit/incidents/1/command",
                "ops.cockpit.write",
            ),
            (Method::GET, "/api/v1/tickets", "tickets.read"),
            (Method::GET, "/api/v1/tickets/1", "tickets.read"),
            (
                Method::GET,
                "/api/v1/tickets/escalation/policy",
                "tickets.read",
            ),
            (
                Method::GET,
                "/api/v1/tickets/escalation/queue",
                "tickets.read",
            ),
            (Method::POST, "/api/v1/tickets", "tickets.write"),
            (Method::PATCH, "/api/v1/tickets/1/status", "tickets.write"),
            (
                Method::PUT,
                "/api/v1/tickets/escalation/policy",
                "tickets.write",
            ),
            (
                Method::POST,
                "/api/v1/tickets/escalation/policy/preview",
                "tickets.write",
            ),
            (
                Method::POST,
                "/api/v1/tickets/escalation/run",
                "tickets.write",
            ),
            (
                Method::DELETE,
                "/api/v1/cmdb/relations/1",
                "cmdb.relations.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/jobs",
                "cmdb.discovery.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/jobs",
                "cmdb.discovery.write",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/jobs/1/run",
                "cmdb.discovery.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/candidates",
                "cmdb.discovery.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/candidates/1/approve",
                "cmdb.discovery.write",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/candidates/1/reject",
                "cmdb.discovery.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/events",
                "cmdb.discovery.read",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/notification-deliveries",
                "cmdb.discovery.read",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/notification-channels",
                "cmdb.notifications.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/notification-channels",
                "cmdb.notifications.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/notification-templates",
                "cmdb.notifications.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/notification-templates",
                "cmdb.notifications.write",
            ),
            (
                Method::GET,
                "/api/v1/cmdb/discovery/notification-subscriptions",
                "cmdb.notifications.read",
            ),
            (
                Method::POST,
                "/api/v1/cmdb/discovery/notification-subscriptions",
                "cmdb.notifications.write",
            ),
            (
                Method::GET,
                "/api/v1/monitoring/sources",
                "monitoring.sources.read",
            ),
            (
                Method::GET,
                "/api/v1/monitoring/overview",
                "monitoring.sources.read",
            ),
            (
                Method::GET,
                "/api/v1/monitoring/layers/service",
                "monitoring.sources.read",
            ),
            (
                Method::GET,
                "/api/v1/monitoring/metrics",
                "monitoring.sources.read",
            ),
            (
                Method::GET,
                "/api/v1/streams/sse",
                "monitoring.sources.read",
            ),
            (
                Method::GET,
                "/api/v1/streams/metrics",
                "monitoring.sources.read",
            ),
            (Method::GET, "/api/v1/setup/preflight", "ops.setup.read"),
            (Method::GET, "/api/v1/setup/checklist", "ops.setup.read"),
            (Method::GET, "/api/v1/setup/templates", "ops.setup.read"),
            (Method::GET, "/api/v1/setup/profiles", "ops.setup.read"),
            (
                Method::GET,
                "/api/v1/setup/profiles/history",
                "ops.setup.read",
            ),
            (
                Method::POST,
                "/api/v1/setup/templates/identity-safe-baseline/preview",
                "ops.setup.write",
            ),
            (
                Method::POST,
                "/api/v1/setup/templates/identity-safe-baseline/apply",
                "ops.setup.write",
            ),
            (
                Method::POST,
                "/api/v1/setup/profiles/smb-small-office/preview",
                "ops.setup.read",
            ),
            (
                Method::POST,
                "/api/v1/setup/profiles/smb-small-office/apply",
                "ops.setup.write",
            ),
            (
                Method::POST,
                "/api/v1/setup/profiles/history/1/revert",
                "ops.setup.write",
            ),
            (Method::GET, "/api/v1/alerts", "alerts.read"),
            (Method::GET, "/api/v1/alerts/1", "alerts.read"),
            (Method::GET, "/api/v1/alerts/1/remediation", "alerts.read"),
            (Method::GET, "/api/v1/alerts/policies", "alerts.read"),
            (Method::POST, "/api/v1/alerts/1/ack", "alerts.write"),
            (Method::POST, "/api/v1/alerts/1/close", "alerts.write"),
            (Method::POST, "/api/v1/alerts/bulk/ack", "alerts.write"),
            (Method::PATCH, "/api/v1/alerts/policies/1", "alerts.write"),
            (
                Method::POST,
                "/api/v1/monitoring/sources",
                "monitoring.sources.write",
            ),
            (
                Method::POST,
                "/api/v1/monitoring/sources/1/probe",
                "monitoring.sources.write",
            ),
            (Method::GET, "/api/v1/iam/users", "system.admin"),
            (Method::POST, "/api/v1/iam/users", "system.admin"),
            (Method::PATCH, "/api/v1/iam/users/1", "system.admin"),
            (Method::GET, "/api/v1/iam/roles", "system.admin"),
            (Method::POST, "/api/v1/iam/roles", "system.admin"),
            (Method::PATCH, "/api/v1/iam/roles/1", "system.admin"),
            (Method::GET, "/api/v1/audit/logs", "system.admin"),
        ];

        for (method, path, permission) in coverage {
            assert_permission(method, path, permission);
        }
    }

    #[test]
    fn denies_lookalike_prefix_paths_by_default() {
        assert!(required_permission(&Method::GET, "/api/v1/cmdb/assetsx").is_none());
        assert!(required_permission(&Method::GET, "/api/v1/iam/usersx").is_none());
        assert!(required_permission(&Method::GET, "/api/v1/audit/logsx").is_none());
    }

    #[test]
    fn rejects_unknown_path() {
        assert!(required_permission(&Method::GET, "/api/v1/cmdb/unknown").is_none());
    }

    #[test]
    fn reads_bearer_token_from_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token-123"));
        let token = read_bearer_token(&headers).expect("bearer token should parse");
        assert_eq!(token.as_deref(), Some("token-123"));
    }

    #[test]
    fn rejects_non_bearer_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc123"));
        let err = read_bearer_token(&headers).expect_err("non-bearer should fail");
        assert_eq!(
            err.to_string(),
            "authorization header must use Bearer token"
        );
    }
}
