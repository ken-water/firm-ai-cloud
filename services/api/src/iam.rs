use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/{user_id}", patch(update_user))
        .route("/users/{user_id}/roles", get(list_user_roles))
        .route(
            "/users/{user_id}/roles/{role_id}",
            post(bind_user_role).delete(unbind_user_role),
        )
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/{role_id}", patch(update_role))
}

#[derive(Debug, Serialize)]
struct IamUser {
    id: i64,
    username: String,
    display_name: Option<String>,
    email: Option<String>,
    auth_source: String,
    is_enabled: bool,
    role_keys: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct IamRole {
    id: i64,
    role_key: String,
    name: String,
    is_system: bool,
    permission_keys: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct IamUserRoleBinding {
    role_id: i64,
    role_key: String,
    role_name: String,
    is_system: bool,
    bound_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RoleBindingActionResponse {
    user_id: i64,
    role_id: i64,
    status: &'static str,
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    username: String,
    display_name: Option<String>,
    email: Option<String>,
    auth_source: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateUserRequest {
    display_name: Option<String>,
    email: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateRoleRequest {
    role_key: String,
    name: String,
    is_system: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateRoleRequest {
    name: Option<String>,
    is_system: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct ListUsersQuery {
    username: Option<String>,
    auth_source: Option<String>,
    is_enabled: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ListRolesQuery {
    role_key: Option<String>,
    is_system: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    username: String,
    display_name: Option<String>,
    email: Option<String>,
    auth_source: String,
    is_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct UserRoleRow {
    user_id: i64,
    role_key: String,
}

#[derive(Debug, FromRow)]
struct RoleRow {
    id: i64,
    role_key: String,
    name: String,
    is_system: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RolePermissionRow {
    role_id: i64,
    permission_key: String,
}

#[derive(Debug, FromRow)]
struct UserRoleBindingRow {
    role_id: i64,
    role_key: String,
    role_name: String,
    is_system: bool,
    bound_at: DateTime<Utc>,
}

async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> AppResult<Json<Vec<IamUser>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, username, display_name, email, auth_source, is_enabled, created_at, updated_at
         FROM iam_users
         WHERE 1=1",
    );

    if let Some(username) = trim_optional(query.username) {
        builder
            .push(" AND username ILIKE ")
            .push_bind(format!("%{username}%"));
    }
    if let Some(auth_source) = trim_optional(query.auth_source) {
        builder.push(" AND auth_source = ").push_bind(auth_source);
    }
    if let Some(is_enabled) = query.is_enabled {
        builder.push(" AND is_enabled = ").push_bind(is_enabled);
    }

    let limit = query.limit.unwrap_or(100).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    builder
        .push(" ORDER BY id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<UserRow> = builder.build_query_as().fetch_all(&state.db).await?;
    if rows.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let user_ids: Vec<i64> = rows.iter().map(|item| item.id).collect();
    let role_map = load_user_roles_map(&state, &user_ids).await?;

    let items = rows
        .into_iter()
        .map(|item| IamUser {
            id: item.id,
            username: item.username,
            display_name: item.display_name,
            email: item.email,
            auth_source: item.auth_source,
            is_enabled: item.is_enabled,
            role_keys: role_map.get(&item.id).cloned().unwrap_or_default(),
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
        .collect();

    Ok(Json(items))
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> AppResult<Json<IamUser>> {
    let username = normalize_username(payload.username)?;
    let display_name = trim_optional(payload.display_name);
    let email = normalize_email(payload.email)?;
    let auth_source = normalize_auth_source(payload.auth_source)?;
    let is_enabled = payload.is_enabled.unwrap_or(true);

    let row: UserRow = sqlx::query_as(
        "INSERT INTO iam_users (username, display_name, email, auth_source, is_enabled)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, username, display_name, email, auth_source, is_enabled, created_at, updated_at",
    )
    .bind(username)
    .bind(display_name)
    .bind(email)
    .bind(auth_source)
    .bind(is_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_user_conflict)?;

    Ok(Json(IamUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        email: row.email,
        auth_source: row.auth_source,
        is_enabled: row.is_enabled,
        role_keys: Vec::new(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Json(payload): Json<UpdateUserRequest>,
) -> AppResult<Json<IamUser>> {
    ensure_user_exists(&state, user_id).await?;

    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE iam_users SET ");
    let mut separated = builder.separated(", ");
    let mut has_changes = false;

    if let Some(display_name) = payload.display_name {
        has_changes = true;
        separated
            .push("display_name = ")
            .push_bind(trim_optional(Some(display_name)));
    }

    if let Some(email) = payload.email {
        has_changes = true;
        separated
            .push("email = ")
            .push_bind(normalize_email(Some(email))?);
    }

    if let Some(is_enabled) = payload.is_enabled {
        has_changes = true;
        separated.push("is_enabled = ").push_bind(is_enabled);
    }

    if !has_changes {
        return Err(AppError::Validation(
            "at least one updatable user field is required".to_string(),
        ));
    }

    separated.push("updated_at = NOW()");
    drop(separated);

    builder.push(" WHERE id = ").push_bind(user_id).push(
        " RETURNING id, username, display_name, email, auth_source, is_enabled, created_at, updated_at",
    );

    builder
        .build_query_as::<UserRow>()
        .fetch_one(&state.db)
        .await
        .map_err(map_user_conflict)?;

    let item = load_user_by_id(&state, user_id).await?;
    Ok(Json(item))
}

async fn list_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Vec<IamUserRoleBinding>>> {
    ensure_user_exists(&state, user_id).await?;

    let rows: Vec<UserRoleBindingRow> = sqlx::query_as(
        "SELECT r.id AS role_id,
                r.role_key,
                r.name AS role_name,
                r.is_system,
                ur.created_at AS bound_at
         FROM iam_user_roles ur
         INNER JOIN iam_roles r ON r.id = ur.role_id
         WHERE ur.user_id = $1
         ORDER BY r.role_key",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|item| IamUserRoleBinding {
            role_id: item.role_id,
            role_key: item.role_key,
            role_name: item.role_name,
            is_system: item.is_system,
            bound_at: item.bound_at,
        })
        .collect();

    Ok(Json(items))
}

async fn bind_user_role(
    State(state): State<AppState>,
    Path((user_id, role_id)): Path<(i64, i64)>,
) -> AppResult<Json<RoleBindingActionResponse>> {
    ensure_user_exists(&state, user_id).await?;
    ensure_role_exists(&state, role_id).await?;

    let result = sqlx::query(
        "INSERT INTO iam_user_roles (user_id, role_id)
         VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(role_id)
    .execute(&state.db)
    .await?;

    let status = if result.rows_affected() > 0 {
        "bound"
    } else {
        "already_bound"
    };

    Ok(Json(RoleBindingActionResponse {
        user_id,
        role_id,
        status,
    }))
}

async fn unbind_user_role(
    State(state): State<AppState>,
    Path((user_id, role_id)): Path<(i64, i64)>,
) -> AppResult<Json<RoleBindingActionResponse>> {
    ensure_user_exists(&state, user_id).await?;
    ensure_role_exists(&state, role_id).await?;

    let result = sqlx::query(
        "DELETE FROM iam_user_roles
         WHERE user_id = $1
           AND role_id = $2",
    )
    .bind(user_id)
    .bind(role_id)
    .execute(&state.db)
    .await?;

    let status = if result.rows_affected() > 0 {
        "unbound"
    } else {
        "not_bound"
    };

    Ok(Json(RoleBindingActionResponse {
        user_id,
        role_id,
        status,
    }))
}

async fn list_roles(
    State(state): State<AppState>,
    Query(query): Query<ListRolesQuery>,
) -> AppResult<Json<Vec<IamRole>>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, role_key, name, is_system, created_at, updated_at
         FROM iam_roles
         WHERE 1=1",
    );

    if let Some(role_key) = trim_optional(query.role_key) {
        builder
            .push(" AND role_key ILIKE ")
            .push_bind(format!("%{role_key}%"));
    }
    if let Some(is_system) = query.is_system {
        builder.push(" AND is_system = ").push_bind(is_system);
    }

    let limit = query.limit.unwrap_or(100).min(500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;
    builder
        .push(" ORDER BY id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RoleRow> = builder.build_query_as().fetch_all(&state.db).await?;
    if rows.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let role_ids: Vec<i64> = rows.iter().map(|item| item.id).collect();
    let permission_map = load_role_permissions_map(&state, &role_ids).await?;

    let items = rows
        .into_iter()
        .map(|item| IamRole {
            id: item.id,
            role_key: item.role_key,
            name: item.name,
            is_system: item.is_system,
            permission_keys: permission_map.get(&item.id).cloned().unwrap_or_default(),
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
        .collect();

    Ok(Json(items))
}

async fn create_role(
    State(state): State<AppState>,
    Json(payload): Json<CreateRoleRequest>,
) -> AppResult<Json<IamRole>> {
    let role_key = normalize_role_key(payload.role_key)?;
    let name = required_trimmed("name", payload.name)?;
    let is_system = payload.is_system.unwrap_or(false);

    let row: RoleRow = sqlx::query_as(
        "INSERT INTO iam_roles (role_key, name, is_system)
         VALUES ($1, $2, $3)
         RETURNING id, role_key, name, is_system, created_at, updated_at",
    )
    .bind(role_key)
    .bind(name)
    .bind(is_system)
    .fetch_one(&state.db)
    .await
    .map_err(map_role_conflict)?;

    Ok(Json(IamRole {
        id: row.id,
        role_key: row.role_key,
        name: row.name,
        is_system: row.is_system,
        permission_keys: Vec::new(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<i64>,
    Json(payload): Json<UpdateRoleRequest>,
) -> AppResult<Json<IamRole>> {
    ensure_role_exists(&state, role_id).await?;

    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE iam_roles SET ");
    let mut separated = builder.separated(", ");
    let mut has_changes = false;

    if let Some(name) = payload.name {
        has_changes = true;
        separated
            .push("name = ")
            .push_bind(required_trimmed("name", name)?);
    }

    if let Some(is_system) = payload.is_system {
        has_changes = true;
        separated.push("is_system = ").push_bind(is_system);
    }

    if !has_changes {
        return Err(AppError::Validation(
            "at least one updatable role field is required".to_string(),
        ));
    }

    separated.push("updated_at = NOW()");
    drop(separated);

    builder
        .push(" WHERE id = ")
        .push_bind(role_id)
        .push(" RETURNING id, role_key, name, is_system, created_at, updated_at");

    builder
        .build_query_as::<RoleRow>()
        .fetch_one(&state.db)
        .await
        .map_err(map_role_conflict)?;

    let item = load_role_by_id(&state, role_id).await?;
    Ok(Json(item))
}

async fn load_user_by_id(state: &AppState, user_id: i64) -> AppResult<IamUser> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, display_name, email, auth_source, is_enabled, created_at, updated_at
         FROM iam_users
         WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound(format!("user {user_id} not found")))?;
    let role_map = load_user_roles_map(state, &[user_id]).await?;

    Ok(IamUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        email: row.email,
        auth_source: row.auth_source,
        is_enabled: row.is_enabled,
        role_keys: role_map.get(&row.id).cloned().unwrap_or_default(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_role_by_id(state: &AppState, role_id: i64) -> AppResult<IamRole> {
    let row: Option<RoleRow> = sqlx::query_as(
        "SELECT id, role_key, name, is_system, created_at, updated_at
         FROM iam_roles
         WHERE id = $1",
    )
    .bind(role_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound(format!("role {role_id} not found")))?;
    let permission_map = load_role_permissions_map(state, &[role_id]).await?;

    Ok(IamRole {
        id: row.id,
        role_key: row.role_key,
        name: row.name,
        is_system: row.is_system,
        permission_keys: permission_map.get(&row.id).cloned().unwrap_or_default(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_user_roles_map(
    state: &AppState,
    user_ids: &[i64],
) -> AppResult<HashMap<i64, Vec<String>>> {
    let rows: Vec<UserRoleRow> = sqlx::query_as(
        "SELECT ur.user_id, r.role_key
         FROM iam_user_roles ur
         INNER JOIN iam_roles r ON r.id = ur.role_id
         WHERE ur.user_id = ANY($1)
         ORDER BY r.role_key",
    )
    .bind(user_ids)
    .fetch_all(&state.db)
    .await?;

    let mut map = HashMap::<i64, Vec<String>>::new();
    for row in rows {
        map.entry(row.user_id).or_default().push(row.role_key);
    }
    Ok(map)
}

async fn load_role_permissions_map(
    state: &AppState,
    role_ids: &[i64],
) -> AppResult<HashMap<i64, Vec<String>>> {
    let rows: Vec<RolePermissionRow> = sqlx::query_as(
        "SELECT rp.role_id, p.permission_key
         FROM iam_role_permissions rp
         INNER JOIN iam_permissions p ON p.id = rp.permission_id
         WHERE rp.role_id = ANY($1)
         ORDER BY p.permission_key",
    )
    .bind(role_ids)
    .fetch_all(&state.db)
    .await?;

    let mut map = HashMap::<i64, Vec<String>>::new();
    for row in rows {
        map.entry(row.role_id).or_default().push(row.permission_key);
    }
    Ok(map)
}

async fn ensure_user_exists(state: &AppState, user_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM iam_users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!("user {user_id} not found")));
    }
    Ok(())
}

async fn ensure_role_exists(state: &AppState, role_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM iam_roles WHERE id = $1)")
        .bind(role_id)
        .fetch_one(&state.db)
        .await?;

    if !exists {
        return Err(AppError::NotFound(format!("role {role_id} not found")));
    }
    Ok(())
}

fn normalize_username(value: String) -> AppResult<String> {
    let normalized = required_trimmed("username", value)?.to_ascii_lowercase();
    if normalized.len() > 128 {
        return Err(AppError::Validation(
            "username length must be <= 128".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '@'))
    {
        return Err(AppError::Validation(
            "username contains invalid characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_role_key(value: String) -> AppResult<String> {
    let normalized = required_trimmed("role_key", value)?.to_ascii_lowercase();
    if normalized.len() > 64 {
        return Err(AppError::Validation(
            "role_key length must be <= 64".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(AppError::Validation(
            "role_key contains invalid characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_email(value: Option<String>) -> AppResult<Option<String>> {
    let Some(email) = trim_optional(value) else {
        return Ok(None);
    };

    if email.len() > 256 {
        return Err(AppError::Validation(
            "email length must be <= 256".to_string(),
        ));
    }

    if !email.contains('@') {
        return Err(AppError::Validation("email format is invalid".to_string()));
    }

    Ok(Some(email))
}

fn normalize_auth_source(value: Option<String>) -> AppResult<String> {
    let normalized = value
        .unwrap_or_else(|| "local".to_string())
        .trim()
        .to_ascii_lowercase();

    match normalized.as_str() {
        "local" | "oidc" | "ldap" | "bootstrap" => Ok(normalized),
        _ => Err(AppError::Validation(
            "auth_source must be one of: local, oidc, ldap, bootstrap".to_string(),
        )),
    }
}

fn required_trimmed(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
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

fn map_user_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("user already exists".to_string());
        }
    }
    AppError::Database(err)
}

fn map_role_conflict(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return AppError::Validation("role already exists".to_string());
        }
    }
    AppError::Database(err)
}
