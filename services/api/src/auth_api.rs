use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::{read_bearer_token, resolve_auth_user},
    error::{AppError, AppResult},
    state::{AppState, OidcSettings},
};

const OIDC_AUTH_SOURCE: &str = "oidc";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/start", get(start_oidc_login))
        .route("/oidc/callback", get(handle_oidc_callback))
        .route("/me", get(get_current_identity))
        .route("/logout", post(logout_current_session))
}

#[derive(Debug, Deserialize)]
struct OidcStartQuery {
    return_to: Option<String>,
}

#[derive(Debug, Serialize)]
struct OidcStartResponse {
    authorization_url: String,
    state: String,
    expires_at: DateTime<Utc>,
    dev_mode_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct OidcCallbackQuery {
    code: String,
    state: String,
}

#[derive(Debug, Serialize)]
struct OidcCallbackResponse {
    access_token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    user: AuthUserSummary,
    roles: Vec<String>,
    return_to: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuthUserSummary {
    id: i64,
    username: String,
    display_name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Serialize)]
struct CurrentIdentityResponse {
    user: AuthUserSummary,
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LogoutResponse {
    revoked: bool,
}

#[derive(Debug, Deserialize)]
struct OidcTokenResponse {
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcUserInfo {
    sub: String,
    email: Option<String>,
    preferred_username: Option<String>,
    name: Option<String>,
}

#[derive(Debug, FromRow)]
struct OidcLoginState {
    state: String,
    return_to: Option<String>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
struct LocalUser {
    id: i64,
    username: String,
    display_name: Option<String>,
    email: Option<String>,
    is_enabled: bool,
}

#[derive(Debug, FromRow)]
struct RoleKeyRow {
    role_key: String,
}

async fn start_oidc_login(
    State(state): State<AppState>,
    Query(query): Query<OidcStartQuery>,
) -> AppResult<Json<OidcStartResponse>> {
    let oidc = ensure_oidc_enabled(&state)?;

    let state_token = Uuid::new_v4().to_string();
    let nonce = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(10);
    let return_to = trim_optional(query.return_to);
    let authorization_url = build_authorization_url(oidc, &state_token, &nonce)?;

    sqlx::query(
        "INSERT INTO auth_oidc_login_states (state, nonce, return_to, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&state_token)
    .bind(nonce)
    .bind(return_to)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Json(OidcStartResponse {
        authorization_url,
        state: state_token,
        expires_at,
        dev_mode_enabled: oidc.dev_mode_enabled,
    }))
}

async fn handle_oidc_callback(
    State(state): State<AppState>,
    Query(query): Query<OidcCallbackQuery>,
) -> AppResult<Json<OidcCallbackResponse>> {
    let oidc = ensure_oidc_enabled(&state)?;
    let code = required_trimmed("code", query.code)?;
    let state_token = required_trimmed("state", query.state)?;

    let login_state = load_pending_oidc_state(&state, &state_token).await?;
    if login_state.expires_at < Utc::now() {
        return Err(AppError::Validation("oidc state is expired".to_string()));
    }

    let profile = resolve_oidc_profile(oidc, &code).await?;
    let actor = format!("oidc:{}", profile.sub);

    let mapped_user = match map_oidc_profile_to_user(&state, &profile).await {
        Ok(user) => user,
        Err(err) => {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor,
                    action: "auth.oidc.callback".to_string(),
                    target_type: "oidc".to_string(),
                    target_id: Some(profile.sub.clone()),
                    result: "failed".to_string(),
                    message: Some(err.to_string()),
                    metadata: json!({
                        "email": profile.email,
                        "preferred_username": profile.preferred_username
                    }),
                },
            )
            .await;
            return Err(err);
        }
    };

    if !mapped_user.is_enabled {
        return Err(AppError::Forbidden(format!(
            "mapped user '{}' is disabled",
            mapped_user.username
        )));
    }

    let roles = load_user_roles(&state, mapped_user.id).await?;
    if roles.is_empty() {
        return Err(AppError::Forbidden(
            "OIDC user is mapped but has no role binding; ask admin to bind a role in /api/v1/iam/users/{id}/roles/{role_id}".to_string(),
        ));
    }

    let session_id = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(state.oidc.session_ttl_minutes as i64);
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, auth_source, expires_at, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&session_id)
    .bind(mapped_user.id)
    .bind(OIDC_AUTH_SOURCE)
    .bind(expires_at)
    .bind(json!({
        "sub": profile.sub,
        "email": profile.email
    }))
    .execute(&state.db)
    .await?;

    sqlx::query(
        "UPDATE auth_oidc_login_states
         SET consumed_at = NOW(),
             consumed_by_user_id = $2
         WHERE state = $1",
    )
    .bind(&login_state.state)
    .bind(mapped_user.id)
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: mapped_user.username.clone(),
            action: "auth.oidc.callback".to_string(),
            target_type: "auth_session".to_string(),
            target_id: Some(session_id.clone()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "user_id": mapped_user.id,
                "roles": roles
            }),
        },
    )
    .await;

    Ok(Json(OidcCallbackResponse {
        access_token: session_id,
        token_type: "Bearer",
        expires_at,
        user: AuthUserSummary {
            id: mapped_user.id,
            username: mapped_user.username,
            display_name: mapped_user.display_name,
            email: mapped_user.email,
        },
        roles,
        return_to: login_state.return_to,
    }))
}

async fn get_current_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<CurrentIdentityResponse>> {
    let username = resolve_auth_user(&state, &headers).await?;
    let user = load_user_by_username(&state, &username).await?;
    let roles = load_user_roles(&state, user.id).await?;

    Ok(Json(CurrentIdentityResponse {
        user: AuthUserSummary {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            email: user.email,
        },
        roles,
    }))
}

async fn logout_current_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<LogoutResponse>> {
    let username = resolve_auth_user(&state, &headers).await?;
    let token = read_bearer_token(&headers)?
        .ok_or_else(|| AppError::Validation("bearer token is required for logout".to_string()))?;

    let result = sqlx::query(
        "UPDATE auth_sessions
         SET revoked_at = NOW()
         WHERE id = $1
           AND revoked_at IS NULL
           AND expires_at > NOW()",
    )
    .bind(&token)
    .execute(&state.db)
    .await?;

    let revoked = result.rows_affected() > 0;
    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: username,
            action: "auth.logout".to_string(),
            target_type: "auth_session".to_string(),
            target_id: Some(token),
            result: if revoked {
                "success".to_string()
            } else {
                "noop".to_string()
            },
            message: None,
            metadata: json!({}),
        },
    )
    .await;

    Ok(Json(LogoutResponse { revoked }))
}

fn ensure_oidc_enabled(state: &AppState) -> AppResult<&OidcSettings> {
    if !state.oidc.enabled {
        return Err(AppError::Validation(
            "oidc is disabled; set AUTH_OIDC_ENABLED=true".to_string(),
        ));
    }
    Ok(&state.oidc)
}

fn build_authorization_url(
    oidc: &OidcSettings,
    state_token: &str,
    nonce: &str,
) -> AppResult<String> {
    if oidc.dev_mode_enabled && oidc.authorization_endpoint.is_none() {
        let redirect_uri = required_setting(&oidc.redirect_uri, "AUTH_OIDC_REDIRECT_URI")?;
        let mut url = Url::parse(redirect_uri).map_err(|_| {
            AppError::Validation("AUTH_OIDC_REDIRECT_URI is invalid URL".to_string())
        })?;
        url.query_pairs_mut()
            .append_pair("code", "dev::demo-sub::demo.user@example.local::Demo User")
            .append_pair("state", state_token);
        return Ok(url.to_string());
    }

    let authorization_endpoint = required_setting(
        &oidc.authorization_endpoint,
        "AUTH_OIDC_AUTHORIZATION_ENDPOINT",
    )?;
    let redirect_uri = required_setting(&oidc.redirect_uri, "AUTH_OIDC_REDIRECT_URI")?;
    let client_id = required_setting(&oidc.client_id, "AUTH_OIDC_CLIENT_ID")?;

    let mut url = Url::parse(authorization_endpoint).map_err(|_| {
        AppError::Validation("AUTH_OIDC_AUTHORIZATION_ENDPOINT is invalid URL".to_string())
    })?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &oidc.scope)
        .append_pair("state", state_token)
        .append_pair("nonce", nonce);

    Ok(url.to_string())
}

async fn load_pending_oidc_state(state: &AppState, state_token: &str) -> AppResult<OidcLoginState> {
    let row: Option<OidcLoginState> = sqlx::query_as(
        "SELECT state, return_to, expires_at
         FROM auth_oidc_login_states
         WHERE state = $1
           AND consumed_at IS NULL",
    )
    .bind(state_token)
    .fetch_optional(&state.db)
    .await?;

    row.ok_or_else(|| AppError::Validation("invalid oidc state".to_string()))
}

async fn resolve_oidc_profile(oidc: &OidcSettings, code: &str) -> AppResult<OidcUserInfo> {
    if oidc.dev_mode_enabled && code.starts_with("dev::") {
        return parse_dev_code(code);
    }

    let token_endpoint = required_setting(&oidc.token_endpoint, "AUTH_OIDC_TOKEN_ENDPOINT")?;
    let userinfo_endpoint =
        required_setting(&oidc.userinfo_endpoint, "AUTH_OIDC_USERINFO_ENDPOINT")?;
    let client_id = required_setting(&oidc.client_id, "AUTH_OIDC_CLIENT_ID")?;
    let client_secret = required_setting(&oidc.client_secret, "AUTH_OIDC_CLIENT_SECRET")?;
    let redirect_uri = required_setting(&oidc.redirect_uri, "AUTH_OIDC_REDIRECT_URI")?;

    let client = Client::new();
    let token_response = client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|err| AppError::Validation(format!("oidc token exchange failed: {err}")))?;

    if !token_response.status().is_success() {
        return Err(AppError::Validation(format!(
            "oidc token exchange failed: HTTP {}",
            token_response.status()
        )));
    }

    let token_payload: OidcTokenResponse = token_response
        .json()
        .await
        .map_err(|_| AppError::Validation("invalid oidc token response payload".to_string()))?;
    let access_token = token_payload.access_token.ok_or_else(|| {
        AppError::Validation("oidc token response missing access_token".to_string())
    })?;

    let userinfo_response = client
        .get(userinfo_endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| AppError::Validation(format!("oidc userinfo request failed: {err}")))?;

    if !userinfo_response.status().is_success() {
        return Err(AppError::Validation(format!(
            "oidc userinfo request failed: HTTP {}",
            userinfo_response.status()
        )));
    }

    let profile: OidcUserInfo = userinfo_response
        .json()
        .await
        .map_err(|_| AppError::Validation("invalid oidc userinfo payload".to_string()))?;

    if profile.sub.trim().is_empty() {
        return Err(AppError::Validation(
            "oidc userinfo payload missing sub".to_string(),
        ));
    }

    Ok(profile)
}

fn parse_dev_code(code: &str) -> AppResult<OidcUserInfo> {
    let mut parts = code.splitn(4, "::");
    let mode = parts.next().unwrap_or_default();
    let sub = parts.next().unwrap_or_default().trim();
    let email = parts.next().unwrap_or_default().trim();
    let name = parts.next().unwrap_or_default().trim();

    if mode != "dev" || sub.is_empty() {
        return Err(AppError::Validation(
            "dev oidc code must be in format dev::<sub>::<email>::<name>".to_string(),
        ));
    }

    let normalized_email = if email.is_empty() {
        None
    } else {
        Some(email.to_ascii_lowercase())
    };

    let preferred_username = normalized_email
        .as_deref()
        .and_then(|value| value.split('@').next())
        .map(ToString::to_string);

    Ok(OidcUserInfo {
        sub: sub.to_string(),
        email: normalized_email,
        preferred_username,
        name: if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        },
    })
}

async fn map_oidc_profile_to_user(
    state: &AppState,
    profile: &OidcUserInfo,
) -> AppResult<LocalUser> {
    if let Some(item) = load_user_by_subject_link(state, &profile.sub).await? {
        if item.is_enabled {
            return Ok(item);
        }
        return Err(AppError::Forbidden(format!(
            "mapped user '{}' is disabled",
            item.username
        )));
    }

    let normalized_email = normalize_email(profile.email.clone());
    if let Some(email) = normalized_email.clone() {
        if let Some(item) = load_user_by_email(state, &email).await? {
            upsert_subject_link(state, &profile.sub, item.id, Some(email)).await?;
            return Ok(item);
        }
    }

    if !state.oidc.auto_provision {
        return Err(AppError::Forbidden(
            "OIDC user is not mapped; ask admin to map by email or subject".to_string(),
        ));
    }

    let username = generate_provision_username(state, profile).await?;
    let display_name = trim_optional(profile.name.clone());
    let created: LocalUser = sqlx::query_as(
        "INSERT INTO iam_users (username, display_name, email, auth_source, is_enabled)
         VALUES ($1, $2, $3, 'oidc', TRUE)
         RETURNING id, username, display_name, email, is_enabled",
    )
    .bind(username)
    .bind(display_name)
    .bind(normalized_email.clone())
    .fetch_one(&state.db)
    .await?;

    upsert_subject_link(state, &profile.sub, created.id, normalized_email).await?;
    Ok(created)
}

async fn load_user_by_subject_link(state: &AppState, sub: &str) -> AppResult<Option<LocalUser>> {
    let user: Option<LocalUser> = sqlx::query_as(
        "SELECT u.id, u.username, u.display_name, u.email, u.is_enabled
         FROM iam_external_identities l
         INNER JOIN iam_users u ON u.id = l.user_id
         WHERE l.auth_source = 'oidc'
           AND l.external_subject = $1
         LIMIT 1",
    )
    .bind(sub)
    .fetch_optional(&state.db)
    .await?;
    Ok(user)
}

async fn load_user_by_email(state: &AppState, email: &str) -> AppResult<Option<LocalUser>> {
    let user: Option<LocalUser> = sqlx::query_as(
        "SELECT id, username, display_name, email, is_enabled
         FROM iam_users
         WHERE lower(email) = lower($1)
         LIMIT 1",
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await?;
    Ok(user)
}

async fn load_user_by_username(state: &AppState, username: &str) -> AppResult<LocalUser> {
    let user: Option<LocalUser> = sqlx::query_as(
        "SELECT id, username, display_name, email, is_enabled
         FROM iam_users
         WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await?;

    user.ok_or_else(|| AppError::Forbidden(format!("user '{username}' does not exist")))
}

async fn load_user_roles(state: &AppState, user_id: i64) -> AppResult<Vec<String>> {
    let rows: Vec<RoleKeyRow> = sqlx::query_as(
        "SELECT r.role_key
         FROM iam_user_roles ur
         INNER JOIN iam_roles r ON r.id = ur.role_id
         WHERE ur.user_id = $1
         ORDER BY r.role_key",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(|item| item.role_key).collect())
}

async fn upsert_subject_link(
    state: &AppState,
    sub: &str,
    user_id: i64,
    email_snapshot: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO iam_external_identities (auth_source, external_subject, user_id, email_snapshot)
         VALUES ('oidc', $1, $2, $3)
         ON CONFLICT (auth_source, external_subject)
         DO UPDATE SET
             user_id = EXCLUDED.user_id,
             email_snapshot = EXCLUDED.email_snapshot,
             updated_at = NOW()",
    )
    .bind(sub)
    .bind(user_id)
    .bind(email_snapshot)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn generate_provision_username(
    state: &AppState,
    profile: &OidcUserInfo,
) -> AppResult<String> {
    let base = choose_base_username(profile)?;

    for index in 0..50 {
        let candidate = if index == 0 {
            base.clone()
        } else {
            format!("{base}-{:02}", index)
        };

        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM iam_users WHERE username = $1)")
                .bind(&candidate)
                .fetch_one(&state.db)
                .await?;
        if !exists {
            return Ok(candidate);
        }
    }

    Err(AppError::Validation(
        "failed to provision unique oidc username".to_string(),
    ))
}

fn choose_base_username(profile: &OidcUserInfo) -> AppResult<String> {
    let raw = profile
        .preferred_username
        .as_ref()
        .or(profile.email.as_ref())
        .map(|item| item.as_str())
        .unwrap_or(profile.sub.as_str());

    let mut base = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            base.push(ch.to_ascii_lowercase());
        }
        if base.len() >= 64 {
            break;
        }
    }

    if base.is_empty() {
        return Err(AppError::Validation(
            "cannot derive username from oidc profile".to_string(),
        ));
    }

    Ok(base)
}

fn required_setting<'a>(value: &'a Option<String>, key: &str) -> AppResult<&'a str> {
    value
        .as_deref()
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} is required when OIDC is enabled")))
}

fn required_trimmed(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    Ok(trimmed.to_string())
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_email(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_lowercase())
        }
    })
}
