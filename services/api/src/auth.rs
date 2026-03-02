use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request},
    middleware::Next,
    response::Response,
};
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

    let user = read_auth_user(request.headers())?;
    let permission = match required_permission(&method, &path) {
        Some(value) => value,
        None => {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: user.to_string(),
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

    let permission_check = check_permission(&state, user, &permission).await?;

    if !permission_check.user_exists {
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: user.to_string(),
                action: "auth.login".to_string(),
                target_type: "user".to_string(),
                target_id: Some(user.to_string()),
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
                actor: user.to_string(),
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
            actor: user.to_string(),
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

fn read_auth_user(headers: &HeaderMap) -> AppResult<&str> {
    let raw = headers
        .get(AUTH_USER_HEADER)
        .ok_or_else(|| AppError::Forbidden(format!("{AUTH_USER_HEADER} header is required")))?;

    let value = raw
        .to_str()
        .map_err(|_| AppError::Forbidden(format!("{AUTH_USER_HEADER} header is invalid")))?;

    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Forbidden(format!(
            "{AUTH_USER_HEADER} header cannot be empty"
        )));
    }

    Ok(value)
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

    let base = if matches_scope(&normalized, "/cmdb/discovery/notification-channels")
        || matches_scope(&normalized, "/cmdb/discovery/notification-templates")
        || matches_scope(&normalized, "/cmdb/discovery/notification-subscriptions")
        || matches_scope(&normalized, "/discovery/notification-channels")
        || matches_scope(&normalized, "/discovery/notification-templates")
        || matches_scope(&normalized, "/discovery/notification-subscriptions")
    {
        "cmdb.notifications"
    } else if matches_scope(&normalized, "/cmdb/discovery")
        || matches_scope(&normalized, "/discovery")
    {
        "cmdb.discovery"
    } else if matches_scope(&normalized, "/cmdb/field-definitions")
        || matches_scope(&normalized, "/field-definitions")
    {
        "cmdb.field_definitions"
    } else if matches_scope(&normalized, "/cmdb/relations")
        || matches_scope(&normalized, "/relations")
    {
        "cmdb.relations"
    } else if matches_scope(&normalized, "/cmdb/assets") || matches_scope(&normalized, "/assets") {
        "cmdb.assets"
    } else if matches_scope(&normalized, "/workflow/requests")
        || matches_scope(&normalized, "/workflows/requests")
    {
        "workflow.requests"
    } else if matches_scope(&normalized, "/workflow/approvals")
        || matches_scope(&normalized, "/workflows/approvals")
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
    use axum::http::Method;

    use super::required_permission;

    fn assert_permission(method: Method, path: &str, expected: &str) {
        assert_eq!(
            required_permission(&method, path).as_deref(),
            Some(expected)
        );
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
    }

    #[test]
    fn permission_matrix_covers_existing_protected_endpoints() {
        let coverage = vec![
            (Method::GET, "/api/v1/cmdb/assets", "cmdb.assets.read"),
            (
                Method::GET,
                "/api/v1/cmdb/assets/by-code/QR-1",
                "cmdb.assets.read",
            ),
            (Method::POST, "/api/v1/cmdb/assets", "cmdb.assets.write"),
            (
                Method::GET,
                "/api/v1/cmdb/assets/1/graph",
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
                Method::POST,
                "/api/v1/cmdb/relations",
                "cmdb.relations.write",
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
}
