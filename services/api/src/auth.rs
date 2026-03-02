use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request},
    middleware::Next,
    response::Response,
};
use sqlx::FromRow;
use tracing::warn;

use crate::{
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
    let permission = required_permission(&method, &path).ok_or_else(|| {
        AppError::Forbidden(format!(
            "no RBAC permission mapping found for route '{path}'"
        ))
    })?;

    let permission_check = check_permission(&state, user, &permission).await?;

    if !permission_check.user_exists {
        return Err(AppError::Forbidden(format!(
            "user '{user}' does not exist or is disabled"
        )));
    }

    if !permission_check.allowed {
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

    if normalized.starts_with("/iam/users")
        || normalized.starts_with("/iam/roles")
        || normalized.starts_with("/users")
        || normalized.starts_with("/roles")
    {
        return Some("system.admin".to_string());
    }

    let base = if normalized.starts_with("/cmdb/discovery/notification-")
        || normalized.starts_with("/discovery/notification-")
    {
        "cmdb.notifications"
    } else if normalized.starts_with("/cmdb/discovery") || normalized.starts_with("/discovery") {
        "cmdb.discovery"
    } else if normalized.starts_with("/cmdb/field-definitions")
        || normalized.starts_with("/field-definitions")
    {
        "cmdb.field_definitions"
    } else if normalized.starts_with("/cmdb/relations") || normalized.starts_with("/relations") {
        "cmdb.relations"
    } else if normalized.starts_with("/cmdb/assets") || normalized.starts_with("/assets") {
        "cmdb.assets"
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

    #[test]
    fn maps_assets_read_permission() {
        assert_eq!(
            required_permission(&Method::GET, "/api/v1/cmdb/assets").as_deref(),
            Some("cmdb.assets.read")
        );
    }

    #[test]
    fn maps_assets_write_permission() {
        assert_eq!(
            required_permission(&Method::POST, "/api/v1/cmdb/assets").as_deref(),
            Some("cmdb.assets.write")
        );
    }

    #[test]
    fn maps_notification_permissions() {
        assert_eq!(
            required_permission(
                &Method::GET,
                "/api/v1/cmdb/discovery/notification-subscriptions"
            )
            .as_deref(),
            Some("cmdb.notifications.read")
        );
    }

    #[test]
    fn maps_discovery_write_permission() {
        assert_eq!(
            required_permission(&Method::POST, "/api/v1/cmdb/discovery/jobs/1/run").as_deref(),
            Some("cmdb.discovery.write")
        );
    }

    #[test]
    fn maps_iam_permission() {
        assert_eq!(
            required_permission(&Method::GET, "/api/v1/iam/users").as_deref(),
            Some("system.admin")
        );
    }

    #[test]
    fn maps_relative_assets_permission() {
        assert_eq!(
            required_permission(&Method::GET, "/assets").as_deref(),
            Some("cmdb.assets.read")
        );
    }

    #[test]
    fn maps_relative_iam_permission() {
        assert_eq!(
            required_permission(&Method::GET, "/users").as_deref(),
            Some("system.admin")
        );
    }

    #[test]
    fn rejects_unknown_path() {
        assert!(required_permission(&Method::GET, "/api/v1/cmdb/unknown").is_none());
    }
}
