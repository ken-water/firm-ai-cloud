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
        AppError::Forbidden(format!("no RBAC permission mapping found for route '{path}'"))
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
    let base = if path.starts_with("/api/v1/cmdb/discovery/notification-") {
        "cmdb.notifications"
    } else if path.starts_with("/api/v1/cmdb/discovery") {
        "cmdb.discovery"
    } else if path.starts_with("/api/v1/cmdb/field-definitions") {
        "cmdb.field_definitions"
    } else if path.starts_with("/api/v1/cmdb/relations") {
        "cmdb.relations"
    } else if path.starts_with("/api/v1/cmdb/assets") {
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
            required_permission(&Method::GET, "/api/v1/cmdb/discovery/notification-subscriptions")
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
    fn rejects_unknown_path() {
        assert!(required_permission(&Method::GET, "/api/v1/cmdb/unknown").is_none());
    }
}
