use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use ldap3::{LdapConnAsync, LdapConnSettings, Scope, SearchEntry, ldap_escape};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::{read_bearer_token, resolve_auth_user},
    error::{AppError, AppResult},
    state::{AppState, LdapSettings, OidcSettings},
};

const OIDC_AUTH_SOURCE: &str = "oidc";
const LDAP_AUTH_SOURCE: &str = "ldap";
const LOCAL_MFA_RECOVERY_CODE_COUNT: usize = 8;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/start", get(start_oidc_login))
        .route("/oidc/callback", get(handle_oidc_callback))
        .route("/local/login", post(login_local_user))
        .route("/local/password", post(set_local_password))
        .route("/local/mfa/enroll", post(enroll_local_mfa))
        .route(
            "/local/mfa/recovery/status",
            get(get_local_mfa_recovery_status),
        )
        .route(
            "/local/mfa/recovery/rotate",
            post(rotate_local_mfa_recovery_codes),
        )
        .route(
            "/local/mfa/recovery/admin-reset",
            post(admin_reset_local_mfa_recovery),
        )
        .route("/ldap/login", post(login_ldap_user))
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
struct LdapLoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LdapLoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    user: AuthUserSummary,
    roles: Vec<String>,
    auth_source: &'static str,
    subject: String,
    groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LocalLoginRequest {
    username: String,
    password: String,
    totp_code: Option<String>,
    recovery_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct LocalLoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    user: AuthUserSummary,
    roles: Vec<String>,
    auth_source: &'static str,
    mfa_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct LocalPasswordSetRequest {
    new_password: String,
}

#[derive(Debug, Serialize)]
struct LocalPasswordSetResponse {
    username: String,
    password_updated: bool,
}

#[derive(Debug, Serialize)]
struct LocalMfaEnrollResponse {
    username: String,
    mfa_enabled: bool,
    totp_secret: String,
    recovery_codes: Vec<String>,
    period_seconds: u32,
    digits: u32,
}

#[derive(Debug, Serialize)]
struct LocalMfaRecoveryStatusResponse {
    username: String,
    remaining_codes: i64,
    consumed_codes: i64,
    revoked_codes: i64,
}

#[derive(Debug, Serialize)]
struct LocalMfaRecoveryRotateResponse {
    username: String,
    generated_codes: usize,
    recovery_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LocalMfaRecoveryAdminResetRequest {
    username: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct LocalMfaRecoveryAdminResetResponse {
    actor: String,
    target_username: String,
    mfa_enabled: bool,
    revoked_codes: i64,
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

#[derive(Debug, Clone)]
struct LdapUserInfo {
    sub: String,
    username: String,
    email: Option<String>,
    display_name: Option<String>,
    groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LdapDevUserRecord {
    username: String,
    password: String,
    sub: String,
    email: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
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

#[derive(Debug, FromRow)]
struct RoleLookupRow {
    id: i64,
    role_key: String,
}

#[derive(Debug, FromRow)]
struct LocalCredentialRow {
    user_id: i64,
    password_salt: String,
    password_hash: String,
    mfa_enabled: bool,
    totp_secret: Option<String>,
    failed_attempts: i32,
    last_failed_at: Option<DateTime<Utc>>,
    locked_until: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct LocalMfaRecoveryStatusRow {
    remaining_codes: i64,
    consumed_codes: i64,
    revoked_codes: i64,
}

enum LocalPasswordCheckResult {
    Verified,
    VerifiedLegacyNeedsMigration { new_salt: String, new_hash: String },
    Invalid,
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

async fn login_ldap_user(
    State(state): State<AppState>,
    Json(payload): Json<LdapLoginRequest>,
) -> AppResult<Json<LdapLoginResponse>> {
    let username = required_trimmed("username", payload.username)?;
    let password = required_trimmed("password", payload.password)?;
    let actor = format!("ldap:{username}");

    let profile = match authenticate_ldap_user(&state, &username, &password).await {
        Ok(profile) => profile,
        Err(err) => {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor,
                    action: "auth.ldap.login".to_string(),
                    target_type: "ldap".to_string(),
                    target_id: Some(username),
                    result: "failed".to_string(),
                    message: Some(err.to_string()),
                    metadata: json!({}),
                },
            )
            .await;
            return Err(err);
        }
    };

    let mapped_user = match map_ldap_profile_to_user(&state, &profile).await {
        Ok(user) => user,
        Err(err) => {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: format!("ldap:{}", profile.username),
                    action: "auth.ldap.login".to_string(),
                    target_type: "ldap".to_string(),
                    target_id: Some(profile.sub.clone()),
                    result: "failed".to_string(),
                    message: Some(err.to_string()),
                    metadata: json!({
                        "email": profile.email,
                        "groups": profile.groups,
                        "mapping_source": "AUTH_LDAP_GROUP_ROLE_MAPPING_JSON"
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

    let roles = sync_ldap_group_mapped_roles(&state, mapped_user.id, &profile.groups).await?;

    let session_id = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(state.oidc.session_ttl_minutes as i64);
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, auth_source, expires_at, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&session_id)
    .bind(mapped_user.id)
    .bind(LDAP_AUTH_SOURCE)
    .bind(expires_at)
    .bind(json!({
        "sub": profile.sub,
        "email": profile.email,
        "groups": profile.groups,
        "username": profile.username
    }))
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: mapped_user.username.clone(),
            action: "auth.ldap.login".to_string(),
            target_type: "auth_session".to_string(),
            target_id: Some(session_id.clone()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "user_id": mapped_user.id,
                "subject": profile.sub,
                "groups": profile.groups,
                "roles": roles,
                "mapping_source": "AUTH_LDAP_GROUP_ROLE_MAPPING_JSON"
            }),
        },
    )
    .await;

    Ok(Json(LdapLoginResponse {
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
        auth_source: LDAP_AUTH_SOURCE,
        subject: profile.sub,
        groups: profile.groups,
    }))
}

async fn set_local_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LocalPasswordSetRequest>,
) -> AppResult<Json<LocalPasswordSetResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let password = required_trimmed("new_password", payload.new_password)?;
    validate_local_password_strength(password.as_str())?;

    let user = load_user_by_username(&state, &actor).await?;
    let (password_salt, password_hash) = hash_local_password_argon2(password.as_str())?;

    sqlx::query(
        "INSERT INTO auth_local_credentials
            (user_id, password_salt, password_hash, mfa_enabled, totp_secret, failed_attempts, last_failed_at, locked_until, updated_at)
         VALUES ($1, $2, $3, FALSE, NULL, 0, NULL, NULL, NOW())
         ON CONFLICT (user_id)
         DO UPDATE SET
            password_salt = EXCLUDED.password_salt,
            password_hash = EXCLUDED.password_hash,
            failed_attempts = 0,
            last_failed_at = NULL,
            locked_until = NULL,
            updated_at = NOW()",
    )
    .bind(user.id)
    .bind(password_salt)
    .bind(password_hash)
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "auth.local.password_set".to_string(),
            target_type: "auth_local_credentials".to_string(),
            target_id: Some(user.id.to_string()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "username": user.username
            }),
        },
    )
    .await;

    Ok(Json(LocalPasswordSetResponse {
        username: actor,
        password_updated: true,
    }))
}

async fn enroll_local_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<LocalMfaEnrollResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let user = load_user_by_username(&state, &actor).await?;
    ensure_local_credential_exists(&state, user.id).await?;

    let totp_secret = build_local_totp_secret();
    sqlx::query(
        "UPDATE auth_local_credentials
         SET mfa_enabled = TRUE,
             totp_secret = $2,
             totp_enrolled_at = NOW(),
             updated_at = NOW()
         WHERE user_id = $1",
    )
    .bind(user.id)
    .bind(&totp_secret)
    .execute(&state.db)
    .await?;
    let recovery_codes =
        issue_local_mfa_recovery_codes(&state, user.id, LOCAL_MFA_RECOVERY_CODE_COUNT).await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "auth.local.mfa_enroll".to_string(),
            target_type: "auth_local_credentials".to_string(),
            target_id: Some(user.id.to_string()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "username": user.username,
                "period_seconds": 30,
                "digits": 6,
                "recovery_codes_generated": recovery_codes.len()
            }),
        },
    )
    .await;

    Ok(Json(LocalMfaEnrollResponse {
        username: actor,
        mfa_enabled: true,
        totp_secret,
        recovery_codes,
        period_seconds: 30,
        digits: 6,
    }))
}

async fn get_local_mfa_recovery_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<LocalMfaRecoveryStatusResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let user = load_user_by_username(&state, &actor).await?;
    ensure_local_credential_exists(&state, user.id).await?;

    let status = load_local_mfa_recovery_status(&state, user.id).await?;
    Ok(Json(LocalMfaRecoveryStatusResponse {
        username: user.username,
        remaining_codes: status.remaining_codes,
        consumed_codes: status.consumed_codes,
        revoked_codes: status.revoked_codes,
    }))
}

async fn rotate_local_mfa_recovery_codes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<LocalMfaRecoveryRotateResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let user = load_user_by_username(&state, &actor).await?;
    ensure_local_credential_exists(&state, user.id).await?;

    let recovery_codes =
        issue_local_mfa_recovery_codes(&state, user.id, LOCAL_MFA_RECOVERY_CODE_COUNT).await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "auth.local.mfa_recovery.rotate".to_string(),
            target_type: "auth_local_recovery_codes".to_string(),
            target_id: Some(user.id.to_string()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "username": user.username,
                "generated_codes": recovery_codes.len()
            }),
        },
    )
    .await;

    Ok(Json(LocalMfaRecoveryRotateResponse {
        username: user.username,
        generated_codes: recovery_codes.len(),
        recovery_codes,
    }))
}

async fn admin_reset_local_mfa_recovery(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LocalMfaRecoveryAdminResetRequest>,
) -> AppResult<Json<LocalMfaRecoveryAdminResetResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    ensure_system_admin_actor(&state, actor.as_str()).await?;

    let target_username = required_trimmed("username", payload.username)?.to_ascii_lowercase();
    let reason = required_trimmed("reason", payload.reason)?;
    let target_user = load_user_by_username(&state, &target_username).await?;
    ensure_local_credential_exists(&state, target_user.id).await?;

    sqlx::query(
        "UPDATE auth_local_credentials
         SET mfa_enabled = FALSE,
             totp_secret = NULL,
             totp_enrolled_at = NULL,
             updated_at = NOW()
         WHERE user_id = $1",
    )
    .bind(target_user.id)
    .execute(&state.db)
    .await?;

    let revoked_codes = revoke_local_mfa_recovery_codes(&state, target_user.id).await?;
    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "auth.local.mfa_recovery.admin_reset".to_string(),
            target_type: "auth_local_credentials".to_string(),
            target_id: Some(target_user.id.to_string()),
            result: "success".to_string(),
            message: Some(reason),
            metadata: json!({
                "target_username": target_user.username,
                "revoked_codes": revoked_codes,
                "mfa_enabled": false
            }),
        },
    )
    .await;

    Ok(Json(LocalMfaRecoveryAdminResetResponse {
        actor,
        target_username: target_user.username,
        mfa_enabled: false,
        revoked_codes,
    }))
}

async fn login_local_user(
    State(state): State<AppState>,
    Json(payload): Json<LocalLoginRequest>,
) -> AppResult<Json<LocalLoginResponse>> {
    let username = required_trimmed("username", payload.username)?.to_ascii_lowercase();
    let password = required_trimmed("password", payload.password)?;
    let totp_code = payload.totp_code.map(|item| item.trim().to_string());
    let recovery_code = payload.recovery_code.map(|item| item.trim().to_string());

    let (fallback_allowed, fallback_reason) = evaluate_local_fallback_policy(
        state.local_auth.fallback_mode.as_str(),
        &state.local_auth.break_glass_users,
        username.as_str(),
    );
    if !fallback_allowed {
        let mode = state.local_auth.fallback_mode.clone();
        let break_glass_users = state.local_auth.break_glass_users.clone();
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: username.clone(),
                action: "auth.local.login".to_string(),
                target_type: "auth_local_policy".to_string(),
                target_id: Some(username.clone()),
                result: "denied".to_string(),
                message: Some(fallback_reason.clone()),
                metadata: json!({
                    "mode": mode,
                    "break_glass_users": break_glass_users
                }),
            },
        )
        .await;
        return Err(AppError::Forbidden(fallback_reason));
    }

    let user = load_user_by_username(&state, &username).await?;
    if !user.is_enabled {
        return Err(AppError::Forbidden(format!(
            "user '{}' is disabled",
            user.username
        )));
    }

    let credential = load_local_credential(&state, user.id)
        .await?
        .ok_or_else(|| {
            AppError::Forbidden("local credential is not configured for this user".to_string())
        })?;

    if let Some(locked_until) = credential.locked_until {
        if locked_until > Utc::now() {
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: username.clone(),
                    action: "auth.local.lockout.blocked".to_string(),
                    target_type: "auth_local_credentials".to_string(),
                    target_id: Some(user.id.to_string()),
                    result: "denied".to_string(),
                    message: Some(format!(
                        "local account locked until {}",
                        locked_until.to_rfc3339()
                    )),
                    metadata: json!({}),
                },
            )
            .await;
            return Err(AppError::Forbidden(format!(
                "local account is locked until {}",
                locked_until.to_rfc3339()
            )));
        }
    }

    let mut legacy_password_migration: Option<(String, String)> = None;
    match verify_local_password(&credential, password.as_str())? {
        LocalPasswordCheckResult::Verified => {}
        LocalPasswordCheckResult::VerifiedLegacyNeedsMigration { new_salt, new_hash } => {
            legacy_password_migration = Some((new_salt, new_hash));
        }
        LocalPasswordCheckResult::Invalid => {
            let lockout_started_at = register_local_login_failure(&state, &credential).await?;
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: username.clone(),
                    action: "auth.local.login".to_string(),
                    target_type: "auth_local_credentials".to_string(),
                    target_id: Some(user.id.to_string()),
                    result: "failed".to_string(),
                    message: Some("invalid local password".to_string()),
                    metadata: json!({}),
                },
            )
            .await;
            if let Some(locked_until) = lockout_started_at {
                write_audit_log_best_effort(
                    &state.db,
                    AuditLogWriteInput {
                        actor: username.clone(),
                        action: "auth.local.lockout.start".to_string(),
                        target_type: "auth_local_credentials".to_string(),
                        target_id: Some(user.id.to_string()),
                        result: "success".to_string(),
                        message: Some(format!(
                            "lockout started until {}",
                            locked_until.to_rfc3339()
                        )),
                        metadata: json!({}),
                    },
                )
                .await;
            }
            return Err(AppError::Forbidden(
                "local authentication failed: invalid credentials".to_string(),
            ));
        }
    }

    if credential.mfa_enabled {
        let mut mfa_passed = false;

        if let Some(provided_code) = totp_code.as_deref().filter(|item| !item.is_empty()) {
            let secret = credential.totp_secret.clone().ok_or_else(|| {
                AppError::Validation("mfa_enabled=true but totp_secret is missing".to_string())
            })?;
            if verify_totp_code(secret.as_str(), provided_code, Utc::now()) {
                mfa_passed = true;
            }
        }

        if !mfa_passed {
            if let Some(provided_recovery_code) =
                recovery_code.as_deref().filter(|item| !item.is_empty())
            {
                let consumed =
                    consume_local_mfa_recovery_code(&state, user.id, provided_recovery_code)
                        .await?;
                if consumed {
                    mfa_passed = true;
                    write_audit_log_best_effort(
                        &state.db,
                        AuditLogWriteInput {
                            actor: username.clone(),
                            action: "auth.local.mfa_recovery.consume".to_string(),
                            target_type: "auth_local_recovery_codes".to_string(),
                            target_id: Some(user.id.to_string()),
                            result: "success".to_string(),
                            message: None,
                            metadata: json!({}),
                        },
                    )
                    .await;
                }
            }
        }

        if !mfa_passed {
            let lockout_started_at = register_local_login_failure(&state, &credential).await?;
            write_audit_log_best_effort(
                &state.db,
                AuditLogWriteInput {
                    actor: username.clone(),
                    action: "auth.local.login".to_string(),
                    target_type: "auth_local_credentials".to_string(),
                    target_id: Some(user.id.to_string()),
                    result: "failed".to_string(),
                    message: Some("invalid mfa verification".to_string()),
                    metadata: json!({
                        "totp_provided": totp_code.as_deref().is_some_and(|item| !item.is_empty()),
                        "recovery_code_provided": recovery_code.as_deref().is_some_and(|item| !item.is_empty())
                    }),
                },
            )
            .await;
            if let Some(locked_until) = lockout_started_at {
                write_audit_log_best_effort(
                    &state.db,
                    AuditLogWriteInput {
                        actor: username.clone(),
                        action: "auth.local.lockout.start".to_string(),
                        target_type: "auth_local_credentials".to_string(),
                        target_id: Some(user.id.to_string()),
                        result: "success".to_string(),
                        message: Some(format!(
                            "lockout started until {}",
                            locked_until.to_rfc3339()
                        )),
                        metadata: json!({}),
                    },
                )
                .await;
            }
            return Err(AppError::Forbidden(
                "local authentication failed: invalid totp_code or recovery_code".to_string(),
            ));
        }
    }

    if let Some((new_salt, new_hash)) = legacy_password_migration {
        sqlx::query(
            "UPDATE auth_local_credentials
             SET password_salt = $2,
                 password_hash = $3,
                 updated_at = NOW()
             WHERE user_id = $1",
        )
        .bind(user.id)
        .bind(new_salt)
        .bind(new_hash)
        .execute(&state.db)
        .await?;

        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: username.clone(),
                action: "auth.local.password_hash.migrated".to_string(),
                target_type: "auth_local_credentials".to_string(),
                target_id: Some(user.id.to_string()),
                result: "success".to_string(),
                message: Some("migrated legacy local password hash to Argon2id".to_string()),
                metadata: json!({
                    "from": "sha256",
                    "to": "argon2id"
                }),
            },
        )
        .await;
    }

    let was_locked = clear_local_login_failures(&state, user.id).await?;
    if was_locked {
        write_audit_log_best_effort(
            &state.db,
            AuditLogWriteInput {
                actor: username.clone(),
                action: "auth.local.lockout.cleared".to_string(),
                target_type: "auth_local_credentials".to_string(),
                target_id: Some(user.id.to_string()),
                result: "success".to_string(),
                message: Some("lockout cleared after successful login".to_string()),
                metadata: json!({}),
            },
        )
        .await;
    }
    enforce_local_session_concurrency_cap(&state, user.id).await?;

    let roles = load_user_roles(&state, user.id).await?;
    if roles.is_empty() {
        return Err(AppError::Forbidden(
            "local user has no role binding".to_string(),
        ));
    }

    let session_id = Uuid::new_v4().to_string();
    let expires_at =
        Utc::now() + Duration::minutes(state.local_auth.session_max_age_minutes as i64);
    sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, auth_source, expires_at, metadata, last_seen_at)
         VALUES ($1, $2, 'local', $3, $4, NOW())",
    )
    .bind(&session_id)
    .bind(user.id)
    .bind(expires_at)
    .bind(json!({
        "mfa_enabled": credential.mfa_enabled,
        "session_idle_timeout_minutes": state.local_auth.session_idle_timeout_minutes
    }))
    .execute(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: username.clone(),
            action: "auth.local.login".to_string(),
            target_type: "auth_session".to_string(),
            target_id: Some(session_id.clone()),
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "user_id": user.id,
                "roles": roles,
                "mfa_enabled": credential.mfa_enabled
            }),
        },
    )
    .await;

    Ok(Json(LocalLoginResponse {
        access_token: session_id,
        token_type: "Bearer",
        expires_at,
        user: AuthUserSummary {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            email: user.email,
        },
        roles,
        auth_source: "local",
        mfa_enabled: credential.mfa_enabled,
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

fn ensure_ldap_enabled(state: &AppState) -> AppResult<&LdapSettings> {
    if !state.ldap.enabled {
        return Err(AppError::Validation(
            "ldap is disabled; set AUTH_LDAP_ENABLED=true".to_string(),
        ));
    }
    Ok(&state.ldap)
}

pub fn validate_ldap_group_role_mapping_config(raw: Option<&str>) -> AppResult<()> {
    let _ = parse_ldap_group_role_mapping(raw)?;
    Ok(())
}

pub fn validate_ldap_live_config(ldap: &LdapSettings) -> AppResult<()> {
    if !ldap.enabled || ldap.mode != "live" {
        return Ok(());
    }

    let live_url = required_setting(&ldap.live_url, "AUTH_LDAP_LIVE_URL")?;
    let live_base_dn = required_setting(&ldap.live_base_dn, "AUTH_LDAP_LIVE_BASE_DN")?;
    if live_base_dn.trim().is_empty() {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_BASE_DN cannot be empty".to_string(),
        ));
    }

    if !(live_url.starts_with("ldap://") || live_url.starts_with("ldaps://")) {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_URL must start with ldap:// or ldaps://".to_string(),
        ));
    }

    if ldap.live_starttls && live_url.starts_with("ldaps://") {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_STARTTLS=true cannot be used with ldaps:// URL".to_string(),
        ));
    }

    if ldap.live_tls_insecure_skip_verify {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_TLS_INSECURE_SKIP_VERIFY=true is not allowed in live mode".to_string(),
        ));
    }

    if ldap.live_user_filter.trim().is_empty() {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_USER_FILTER cannot be empty".to_string(),
        ));
    }
    if !ldap.live_user_filter.contains("{username}") {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_USER_FILTER must include '{username}' placeholder".to_string(),
        ));
    }

    let bind_dn_present = ldap
        .live_bind_dn
        .as_deref()
        .map(|item| !item.trim().is_empty())
        .unwrap_or(false);
    let bind_pw_present = ldap
        .live_bind_password
        .as_deref()
        .map(|item| !item.trim().is_empty())
        .unwrap_or(false);
    if bind_dn_present != bind_pw_present {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_BIND_DN and AUTH_LDAP_LIVE_BIND_PASSWORD must be provided together"
                .to_string(),
        ));
    }

    for (key, value) in [
        ("AUTH_LDAP_LIVE_ATTR_EMAIL", ldap.live_attr_email.as_str()),
        (
            "AUTH_LDAP_LIVE_ATTR_DISPLAY_NAME",
            ldap.live_attr_display_name.as_str(),
        ),
        ("AUTH_LDAP_LIVE_ATTR_GROUPS", ldap.live_attr_groups.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(AppError::Validation(format!("{key} cannot be empty")));
        }
    }

    Ok(())
}

async fn authenticate_ldap_user(
    state: &AppState,
    username: &str,
    password: &str,
) -> AppResult<LdapUserInfo> {
    let ldap = ensure_ldap_enabled(state)?;

    match ldap.mode.as_str() {
        "dev" => {
            let payload = ldap.dev_users_json.as_deref().ok_or_else(|| {
                AppError::Validation(
                    "AUTH_LDAP_DEV_USERS_JSON is required when AUTH_LDAP_MODE=dev".to_string(),
                )
            })?;
            authenticate_ldap_dev_user(payload, username, password)
        }
        "live" => authenticate_ldap_live_user(ldap, username, password).await,
        _ => Err(AppError::Validation(format!(
            "unsupported AUTH_LDAP_MODE '{}'",
            ldap.mode
        ))),
    }
}

fn authenticate_ldap_dev_user(
    dev_users_json: &str,
    username: &str,
    password: &str,
) -> AppResult<LdapUserInfo> {
    let users: Vec<LdapDevUserRecord> = serde_json::from_str(dev_users_json).map_err(|_| {
        AppError::Validation("AUTH_LDAP_DEV_USERS_JSON must be a valid JSON array".to_string())
    })?;

    let normalized_username = username.trim().to_ascii_lowercase();
    let matched = users.into_iter().find(|item| {
        item.username
            .trim()
            .eq_ignore_ascii_case(&normalized_username)
            && item.password == password
    });

    let user = matched.ok_or_else(|| {
        AppError::Forbidden("ldap authentication failed: invalid credentials".to_string())
    })?;

    let subject = required_trimmed("sub", user.sub)?;
    Ok(LdapUserInfo {
        sub: subject,
        username: user.username.trim().to_ascii_lowercase(),
        email: normalize_email(user.email),
        display_name: trim_optional(user.display_name),
        groups: user
            .groups
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
    })
}

async fn authenticate_ldap_live_user(
    ldap: &LdapSettings,
    username: &str,
    password: &str,
) -> AppResult<LdapUserInfo> {
    validate_ldap_live_config(ldap)?;
    let live_url = required_setting(&ldap.live_url, "AUTH_LDAP_LIVE_URL")?;
    let live_base_dn = required_setting(&ldap.live_base_dn, "AUTH_LDAP_LIVE_BASE_DN")?;
    let normalized_username = username.trim().to_ascii_lowercase();
    if normalized_username.is_empty() {
        return Err(AppError::Validation("username cannot be empty".to_string()));
    }
    if password.trim().is_empty() {
        return Err(AppError::Validation("password cannot be empty".to_string()));
    }

    let escaped_username = ldap_escape(normalized_username.as_str()).into_owned();
    let user_filter =
        build_ldap_live_user_filter(ldap.live_user_filter.as_str(), escaped_username.as_str())?;

    let mut settings = LdapConnSettings::new().set_starttls(ldap.live_starttls);
    if ldap.live_tls_insecure_skip_verify {
        settings = settings.set_no_tls_verify(true);
    }

    let (conn, mut client) = LdapConnAsync::with_settings(settings, live_url)
        .await
        .map_err(|_| {
            AppError::Validation(
                "failed to connect to ldap server in AUTH_LDAP_MODE=live".to_string(),
            )
        })?;
    ldap3::drive!(conn);

    if let (Some(bind_dn), Some(bind_password)) = (
        ldap.live_bind_dn.as_deref(),
        ldap.live_bind_password.as_deref(),
    ) {
        let bind_result = client
            .simple_bind(bind_dn, bind_password)
            .await
            .map_err(|_| AppError::Validation("ldap service bind request failed".to_string()))?;
        bind_result
            .success()
            .map_err(|_| AppError::Validation("ldap service bind failed".to_string()))?;
    }

    let attrs = vec![
        ldap.live_attr_email.as_str(),
        ldap.live_attr_display_name.as_str(),
        ldap.live_attr_groups.as_str(),
    ];
    let (entries, _res) = client
        .search(live_base_dn, Scope::Subtree, user_filter.as_str(), attrs)
        .await
        .map_err(|_| AppError::Validation("ldap user search request failed".to_string()))?
        .success()
        .map_err(|_| AppError::Validation("ldap user search failed".to_string()))?;

    if entries.is_empty() {
        let _ = client.unbind().await;
        return Err(AppError::Forbidden(
            "ldap authentication failed: invalid credentials".to_string(),
        ));
    }
    if entries.len() > 1 {
        let _ = client.unbind().await;
        return Err(AppError::Validation(
            "ldap user search returned multiple entries; refine AUTH_LDAP_LIVE_USER_FILTER"
                .to_string(),
        ));
    }

    let search_entry = SearchEntry::construct(
        entries
            .into_iter()
            .next()
            .expect("entry exists when len is checked"),
    );
    let user_dn = search_entry.dn.trim().to_string();
    if user_dn.is_empty() {
        let _ = client.unbind().await;
        return Err(AppError::Validation(
            "ldap user entry DN is empty in search result".to_string(),
        ));
    }

    let user_bind = client
        .simple_bind(user_dn.as_str(), password)
        .await
        .map_err(|_| AppError::Validation("ldap user bind request failed".to_string()))?;
    if user_bind.success().is_err() {
        let _ = client.unbind().await;
        return Err(AppError::Forbidden(
            "ldap authentication failed: invalid credentials".to_string(),
        ));
    }
    let _ = client.unbind().await;

    let email = normalize_email(first_attr_value(
        &search_entry,
        ldap.live_attr_email.as_str(),
    ));
    let display_name = trim_optional(first_attr_value(
        &search_entry,
        ldap.live_attr_display_name.as_str(),
    ));
    let groups = extract_ldap_live_groups(&search_entry, ldap.live_attr_groups.as_str());

    Ok(LdapUserInfo {
        sub: user_dn,
        username: normalized_username,
        email,
        display_name,
        groups,
    })
}

fn build_ldap_live_user_filter(template: &str, escaped_username: &str) -> AppResult<String> {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_USER_FILTER cannot be empty".to_string(),
        ));
    }
    if !trimmed.contains("{username}") {
        return Err(AppError::Validation(
            "AUTH_LDAP_LIVE_USER_FILTER must include '{username}' placeholder".to_string(),
        ));
    }
    Ok(trimmed.replace("{username}", escaped_username))
}

fn first_attr_value(entry: &SearchEntry, attr_key: &str) -> Option<String> {
    entry
        .attrs
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(attr_key))
        .and_then(|(_, values)| values.first().cloned())
}

fn extract_ldap_live_groups(entry: &SearchEntry, attr_key: &str) -> Vec<String> {
    let values = entry
        .attrs
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(attr_key))
        .map(|(_, values)| values.clone())
        .unwrap_or_default();

    let mut groups = BTreeSet::new();
    for value in values {
        let normalized = normalize_ldap_group_value(value.as_str());
        if !normalized.is_empty() {
            groups.insert(normalized);
        }
    }
    groups.into_iter().collect()
}

fn normalize_ldap_group_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let first_component = trimmed.split(',').next().unwrap_or(trimmed).trim();
    if let Some((_, value)) = first_component.split_once('=') {
        let group = value.trim();
        if !group.is_empty() {
            return group.to_ascii_lowercase();
        }
    }

    trimmed.to_ascii_lowercase()
}

fn parse_ldap_group_role_mapping(raw: Option<&str>) -> AppResult<BTreeMap<String, Vec<String>>> {
    let Some(raw) = raw else {
        return Ok(BTreeMap::new());
    };

    let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| {
        AppError::Validation(
            "AUTH_LDAP_GROUP_ROLE_MAPPING_JSON must be a JSON object of group -> role array"
                .to_string(),
        )
    })?;

    let object = value.as_object().ok_or_else(|| {
        AppError::Validation("AUTH_LDAP_GROUP_ROLE_MAPPING_JSON must be a JSON object".to_string())
    })?;

    let mut mapping = BTreeMap::new();
    for (group_key, roles_value) in object {
        let group = group_key.trim().to_ascii_lowercase();
        if group.is_empty() {
            return Err(AppError::Validation(
                "ldap group key cannot be empty".to_string(),
            ));
        }

        let role_array = roles_value.as_array().ok_or_else(|| {
            AppError::Validation(format!(
                "ldap group '{group}' mapping value must be a JSON string array"
            ))
        })?;
        if role_array.is_empty() {
            return Err(AppError::Validation(format!(
                "ldap group '{group}' must map to at least one role"
            )));
        }

        let mut role_set = BTreeSet::new();
        for item in role_array {
            let role_key = item.as_str().ok_or_else(|| {
                AppError::Validation(format!(
                    "ldap group '{group}' role list must contain only strings"
                ))
            })?;
            let normalized_role = role_key.trim().to_ascii_lowercase();
            if normalized_role.is_empty() {
                return Err(AppError::Validation(format!(
                    "ldap group '{group}' contains empty role key"
                )));
            }
            role_set.insert(normalized_role);
        }

        mapping.insert(group, role_set.into_iter().collect());
    }

    Ok(mapping)
}

fn resolve_ldap_role_keys(
    mapping: &BTreeMap<String, Vec<String>>,
    groups: &[String],
) -> Vec<String> {
    let mut resolved = BTreeSet::new();
    for group in groups {
        let normalized_group = group.trim().to_ascii_lowercase();
        if let Some(role_keys) = mapping.get(normalized_group.as_str()) {
            for role_key in role_keys {
                resolved.insert(role_key.clone());
            }
        }
    }
    resolved.into_iter().collect()
}

async fn sync_ldap_group_mapped_roles(
    state: &AppState,
    user_id: i64,
    groups: &[String],
) -> AppResult<Vec<String>> {
    let mapping = parse_ldap_group_role_mapping(state.ldap.group_role_mapping_json.as_deref())?;
    if mapping.is_empty() {
        return Err(AppError::Forbidden(
            "ldap group-role mapping is empty; set AUTH_LDAP_GROUP_ROLE_MAPPING_JSON".to_string(),
        ));
    }

    let resolved_role_keys = resolve_ldap_role_keys(&mapping, groups);
    if resolved_role_keys.is_empty() {
        return Err(AppError::Forbidden(format!(
            "no ldap role mapping matched for groups {:?}",
            groups
        )));
    }

    let rows: Vec<RoleLookupRow> = sqlx::query_as(
        "SELECT id, role_key
         FROM iam_roles
         WHERE role_key = ANY($1)
         ORDER BY role_key",
    )
    .bind(&resolved_role_keys)
    .fetch_all(&state.db)
    .await?;

    let found_role_keys: BTreeSet<String> = rows.iter().map(|item| item.role_key.clone()).collect();
    let expected_role_keys: BTreeSet<String> = resolved_role_keys.iter().cloned().collect();
    if found_role_keys != expected_role_keys {
        let missing: Vec<String> = expected_role_keys
            .difference(&found_role_keys)
            .cloned()
            .collect();
        return Err(AppError::Validation(format!(
            "ldap group-role mapping references unknown iam_roles: {:?}",
            missing
        )));
    }

    let role_ids: Vec<i64> = rows.iter().map(|item| item.id).collect();
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM iam_user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    for role_id in role_ids {
        sqlx::query(
            "INSERT INTO iam_user_roles (user_id, role_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    load_user_roles(state, user_id).await
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
    if let Some(item) = load_user_by_subject_link(state, OIDC_AUTH_SOURCE, &profile.sub).await? {
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
            upsert_subject_link(state, OIDC_AUTH_SOURCE, &profile.sub, item.id, Some(email))
                .await?;
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

    upsert_subject_link(
        state,
        OIDC_AUTH_SOURCE,
        &profile.sub,
        created.id,
        normalized_email,
    )
    .await?;
    Ok(created)
}

async fn map_ldap_profile_to_user(
    state: &AppState,
    profile: &LdapUserInfo,
) -> AppResult<LocalUser> {
    if let Some(item) = load_user_by_subject_link(state, LDAP_AUTH_SOURCE, &profile.sub).await? {
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
            upsert_subject_link(state, LDAP_AUTH_SOURCE, &profile.sub, item.id, Some(email))
                .await?;
            return Ok(item);
        }
    }

    if !state.ldap.auto_provision {
        return Err(AppError::Forbidden(
            "LDAP user is not mapped; ask admin to map by email or subject".to_string(),
        ));
    }

    let username = generate_provision_username_from_hint(
        state,
        Some(profile.username.as_str()),
        normalized_email.as_deref(),
        profile.sub.as_str(),
    )
    .await?;
    let display_name = trim_optional(profile.display_name.clone());
    let created: LocalUser = sqlx::query_as(
        "INSERT INTO iam_users (username, display_name, email, auth_source, is_enabled)
         VALUES ($1, $2, $3, 'ldap', TRUE)
         RETURNING id, username, display_name, email, is_enabled",
    )
    .bind(username)
    .bind(display_name)
    .bind(normalized_email.clone())
    .fetch_one(&state.db)
    .await?;

    upsert_subject_link(
        state,
        LDAP_AUTH_SOURCE,
        &profile.sub,
        created.id,
        normalized_email,
    )
    .await?;
    Ok(created)
}

async fn load_user_by_subject_link(
    state: &AppState,
    auth_source: &str,
    sub: &str,
) -> AppResult<Option<LocalUser>> {
    let user: Option<LocalUser> = sqlx::query_as(
        "SELECT u.id, u.username, u.display_name, u.email, u.is_enabled
         FROM iam_external_identities l
         INNER JOIN iam_users u ON u.id = l.user_id
         WHERE l.auth_source = $1
           AND l.external_subject = $2
         LIMIT 1",
    )
    .bind(auth_source)
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

async fn load_local_credential(
    state: &AppState,
    user_id: i64,
) -> AppResult<Option<LocalCredentialRow>> {
    let row: Option<LocalCredentialRow> = sqlx::query_as(
        "SELECT user_id, password_salt, password_hash, mfa_enabled, totp_secret, failed_attempts, last_failed_at, locked_until
         FROM auth_local_credentials
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row)
}

async fn ensure_local_credential_exists(state: &AppState, user_id: i64) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM auth_local_credentials WHERE user_id = $1)",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(AppError::Validation(
            "local credential is not configured; call /api/v1/auth/local/password first"
                .to_string(),
        ));
    }
    Ok(())
}

async fn ensure_system_admin_actor(state: &AppState, actor: &str) -> AppResult<()> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM iam_users u
            INNER JOIN iam_user_roles ur ON ur.user_id = u.id
            INNER JOIN iam_role_permissions rp ON rp.role_id = ur.role_id
            INNER JOIN iam_permissions p ON p.id = rp.permission_id
            WHERE u.username = $1
              AND u.is_enabled = TRUE
              AND p.permission_key = 'system.admin'
        )",
    )
    .bind(actor)
    .fetch_one(&state.db)
    .await?;
    if !allowed {
        return Err(AppError::Forbidden(
            "system.admin permission is required for admin reset".to_string(),
        ));
    }
    Ok(())
}

async fn issue_local_mfa_recovery_codes(
    state: &AppState,
    user_id: i64,
    count: usize,
) -> AppResult<Vec<String>> {
    if count == 0 {
        return Err(AppError::Validation(
            "recovery code count must be greater than zero".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE auth_local_recovery_codes
         SET revoked_at = NOW()
         WHERE user_id = $1
           AND consumed_at IS NULL
           AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    let mut issued_codes = Vec::with_capacity(count);
    let mut attempt_codes = BTreeSet::new();
    while issued_codes.len() < count {
        let code = build_local_mfa_recovery_code();
        if !attempt_codes.insert(code.clone()) {
            continue;
        }
        let code_hash = hash_local_mfa_recovery_code(code.as_str());
        let inserted = sqlx::query(
            "INSERT INTO auth_local_recovery_codes (user_id, code_hash)
             VALUES ($1, $2)
             ON CONFLICT (user_id, code_hash) DO NOTHING",
        )
        .bind(user_id)
        .bind(code_hash)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 1 {
            issued_codes.push(code);
        }
    }
    tx.commit().await?;

    Ok(issued_codes)
}

async fn revoke_local_mfa_recovery_codes(state: &AppState, user_id: i64) -> AppResult<i64> {
    let result = sqlx::query(
        "UPDATE auth_local_recovery_codes
         SET revoked_at = NOW()
         WHERE user_id = $1
           AND consumed_at IS NULL
           AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    Ok(result.rows_affected() as i64)
}

async fn consume_local_mfa_recovery_code(
    state: &AppState,
    user_id: i64,
    recovery_code: &str,
) -> AppResult<bool> {
    let code_hash = hash_local_mfa_recovery_code(recovery_code);
    let result = sqlx::query(
        "UPDATE auth_local_recovery_codes
         SET consumed_at = NOW()
         WHERE user_id = $1
           AND code_hash = $2
           AND consumed_at IS NULL
           AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(code_hash)
    .execute(&state.db)
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn load_local_mfa_recovery_status(
    state: &AppState,
    user_id: i64,
) -> AppResult<LocalMfaRecoveryStatusRow> {
    let row: LocalMfaRecoveryStatusRow = sqlx::query_as(
        "SELECT
            COUNT(*) FILTER (WHERE consumed_at IS NULL AND revoked_at IS NULL) AS remaining_codes,
            COUNT(*) FILTER (WHERE consumed_at IS NOT NULL) AS consumed_codes,
            COUNT(*) FILTER (WHERE revoked_at IS NOT NULL) AS revoked_codes
         FROM auth_local_recovery_codes
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(row)
}

fn build_local_mfa_recovery_code() -> String {
    let token = Uuid::new_v4().simple().to_string();
    format!(
        "{}-{}-{}",
        token[0..4].to_ascii_uppercase(),
        token[4..8].to_ascii_uppercase(),
        token[8..12].to_ascii_uppercase()
    )
}

fn hash_local_mfa_recovery_code(code: &str) -> String {
    let normalized = code.trim().to_ascii_uppercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
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

fn validate_local_password_strength(password: &str) -> AppResult<()> {
    if password.len() < 10 {
        return Err(AppError::Validation(
            "new_password must be at least 10 characters".to_string(),
        ));
    }
    Ok(())
}

fn build_local_totp_secret() -> String {
    Uuid::new_v4().simple().to_string()
}

fn hash_local_password_sha256_legacy(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn hash_local_password_argon2(password: &str) -> AppResult<(String, String)> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| {
            AppError::Validation("failed to hash local password with Argon2id".to_string())
        })?
        .to_string();
    Ok((salt.to_string(), password_hash))
}

fn verify_argon2_password(password_hash: &str, password: &str) -> bool {
    let parsed = match PasswordHash::new(password_hash) {
        Ok(value) => value,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn is_argon2_password_hash(password_hash: &str) -> bool {
    password_hash
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("$argon2")
}

fn verify_local_password(
    credential: &LocalCredentialRow,
    password: &str,
) -> AppResult<LocalPasswordCheckResult> {
    if is_argon2_password_hash(credential.password_hash.as_str()) {
        if verify_argon2_password(credential.password_hash.as_str(), password) {
            return Ok(LocalPasswordCheckResult::Verified);
        }
        return Ok(LocalPasswordCheckResult::Invalid);
    }

    let expected_hash =
        hash_local_password_sha256_legacy(credential.password_salt.as_str(), password);
    if expected_hash != credential.password_hash {
        return Ok(LocalPasswordCheckResult::Invalid);
    }

    let (new_salt, new_hash) = hash_local_password_argon2(password)?;
    Ok(LocalPasswordCheckResult::VerifiedLegacyNeedsMigration { new_salt, new_hash })
}

async fn register_local_login_failure(
    state: &AppState,
    credential: &LocalCredentialRow,
) -> AppResult<Option<DateTime<Utc>>> {
    let now = Utc::now();
    let window_seconds = state.local_auth.rate_limit_window_seconds as i64;
    let mut attempts = credential.failed_attempts.max(0) as u32;

    if let Some(last_failed_at) = credential.last_failed_at {
        let elapsed = now.signed_duration_since(last_failed_at).num_seconds();
        if elapsed > window_seconds {
            attempts = 0;
        }
    } else {
        attempts = 0;
    }
    attempts = attempts.saturating_add(1);

    let lockout_threshold = state.local_auth.lockout_threshold.max(1);
    let rate_limit_max = state.local_auth.rate_limit_max_attempts.max(1);
    let should_lock = attempts >= lockout_threshold || attempts >= rate_limit_max;
    let locked_until = if should_lock {
        Some(now + Duration::minutes(state.local_auth.lockout_minutes.max(1) as i64))
    } else {
        None
    };

    sqlx::query(
        "UPDATE auth_local_credentials
         SET failed_attempts = $2,
             last_failed_at = $3,
             locked_until = $4,
             updated_at = NOW()
         WHERE user_id = $1",
    )
    .bind(credential.user_id)
    .bind(attempts as i32)
    .bind(now)
    .bind(locked_until)
    .execute(&state.db)
    .await?;

    Ok(locked_until)
}

async fn clear_local_login_failures(state: &AppState, user_id: i64) -> AppResult<bool> {
    let previous_locked_until: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT locked_until
         FROM auth_local_credentials
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    sqlx::query(
        "UPDATE auth_local_credentials
         SET failed_attempts = 0,
             last_failed_at = NULL,
             locked_until = NULL,
             updated_at = NOW()
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    Ok(previous_locked_until.is_some())
}

async fn enforce_local_session_concurrency_cap(state: &AppState, user_id: i64) -> AppResult<()> {
    let cap = state.local_auth.session_max_concurrent.max(1) as usize;
    let sessions: Vec<String> = sqlx::query_scalar(
        "SELECT id
         FROM auth_sessions
         WHERE user_id = $1
           AND auth_source = 'local'
           AND revoked_at IS NULL
           AND expires_at > NOW()
         ORDER BY issued_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let revoke_count = local_session_revoke_count(sessions.len(), cap);
    if revoke_count == 0 {
        return Ok(());
    }

    for session_id in sessions.into_iter().take(revoke_count) {
        sqlx::query(
            "UPDATE auth_sessions
             SET revoked_at = NOW()
             WHERE id = $1
               AND revoked_at IS NULL",
        )
        .bind(session_id)
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

fn local_session_revoke_count(active_sessions: usize, cap: usize) -> usize {
    let effective_cap = cap.max(1);
    if active_sessions < effective_cap {
        0
    } else {
        active_sessions - effective_cap + 1
    }
}

fn verify_totp_code(secret: &str, code: &str, now: DateTime<Utc>) -> bool {
    if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    let current_counter = now.timestamp() / 30;
    for skew in -1..=1 {
        let candidate_counter = current_counter + skew;
        if candidate_counter < 0 {
            continue;
        }
        let expected = compute_totp_code_for_counter(secret.as_bytes(), candidate_counter as u64);
        if expected == code {
            return true;
        }
    }
    false
}

fn compute_totp_code_for_counter(secret: &[u8], counter: u64) -> String {
    let counter_bytes = counter.to_be_bytes();
    let hmac = hmac_sha256(secret, &counter_bytes);
    let offset = (hmac[hmac.len() - 1] & 0x0f) as usize;
    let binary = ((hmac[offset] as u32 & 0x7f) << 24)
        | ((hmac[offset + 1] as u32) << 16)
        | ((hmac[offset + 2] as u32) << 8)
        | (hmac[offset + 3] as u32);
    format!("{:06}", binary % 1_000_000)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut normalized_key = [0u8; 64];
    if key.len() > 64 {
        let digest = Sha256::digest(key);
        normalized_key[..32].copy_from_slice(&digest);
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0u8; 64];
    let mut opad = [0u8; 64];
    for idx in 0..64 {
        ipad[idx] = normalized_key[idx] ^ 0x36;
        opad[idx] = normalized_key[idx] ^ 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let output = outer.finalize();

    let mut result = [0u8; 32];
    result.copy_from_slice(&output);
    result
}

async fn upsert_subject_link(
    state: &AppState,
    auth_source: &str,
    sub: &str,
    user_id: i64,
    email_snapshot: Option<String>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO iam_external_identities (auth_source, external_subject, user_id, email_snapshot)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (auth_source, external_subject)
         DO UPDATE SET
             user_id = EXCLUDED.user_id,
             email_snapshot = EXCLUDED.email_snapshot,
             updated_at = NOW()",
    )
    .bind(auth_source)
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
    generate_provision_username_from_hint(
        state,
        profile.preferred_username.as_deref(),
        profile.email.as_deref(),
        profile.sub.as_str(),
    )
    .await
}

async fn generate_provision_username_from_hint(
    state: &AppState,
    preferred: Option<&str>,
    secondary: Option<&str>,
    fallback: &str,
) -> AppResult<String> {
    let base = choose_base_username(preferred, secondary, fallback)?;
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
        "failed to provision unique username".to_string(),
    ))
}

fn choose_base_username(
    preferred: Option<&str>,
    secondary: Option<&str>,
    fallback: &str,
) -> AppResult<String> {
    let raw = preferred.or(secondary).unwrap_or(fallback);

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
            "cannot derive username from identity profile".to_string(),
        ));
    }

    Ok(base)
}

fn required_setting<'a>(value: &'a Option<String>, key: &str) -> AppResult<&'a str> {
    value
        .as_deref()
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} is required")))
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

#[cfg(test)]
mod tests {
    use super::{
        LocalCredentialRow, LocalPasswordCheckResult, authenticate_ldap_dev_user,
        build_ldap_live_user_filter, build_local_mfa_recovery_code, choose_base_username,
        compute_totp_code_for_counter, hash_local_mfa_recovery_code, hash_local_password_argon2,
        hash_local_password_sha256_legacy, local_session_revoke_count, normalize_ldap_group_value,
        parse_ldap_group_role_mapping, resolve_ldap_role_keys, validate_ldap_live_config,
        verify_local_password, verify_totp_code,
    };
    use crate::{error::AppError, state::LdapSettings};
    use chrono::{TimeZone, Utc};
    use std::collections::{BTreeMap, BTreeSet};

    fn sample_live_ldap_settings() -> LdapSettings {
        LdapSettings {
            enabled: true,
            mode: "live".to_string(),
            auto_provision: false,
            dev_users_json: None,
            group_role_mapping_json: None,
            live_url: Some("ldaps://ldap.example.local:636".to_string()),
            live_bind_dn: Some("cn=svc,ou=svc,dc=example,dc=local".to_string()),
            live_bind_password: Some("svc-pass".to_string()),
            live_base_dn: Some("ou=users,dc=example,dc=local".to_string()),
            live_user_filter: "(&(objectClass=person)(uid={username}))".to_string(),
            live_attr_email: "mail".to_string(),
            live_attr_display_name: "displayName".to_string(),
            live_attr_groups: "memberOf".to_string(),
            live_starttls: false,
            live_tls_insecure_skip_verify: false,
        }
    }

    #[test]
    fn authenticates_ldap_dev_user_successfully() {
        let payload = r#"
[
  {
    "username": "ops-admin",
    "password": "dev-pass-1",
    "sub": "cn=ops-admin,ou=users,dc=example,dc=local",
    "email": "ops-admin@example.local",
    "display_name": "Ops Admin",
    "groups": ["ops-admins", "oncall"]
  }
]
"#;
        let profile =
            authenticate_ldap_dev_user(payload, "ops-admin", "dev-pass-1").expect("must pass");

        assert_eq!(profile.username, "ops-admin");
        assert_eq!(
            profile.sub,
            "cn=ops-admin,ou=users,dc=example,dc=local".to_string()
        );
        assert_eq!(profile.email, Some("ops-admin@example.local".to_string()));
        assert_eq!(profile.groups, vec!["ops-admins", "oncall"]);
    }

    #[test]
    fn rejects_ldap_dev_user_with_invalid_password() {
        let payload = r#"
[
  {
    "username": "ops-admin",
    "password": "dev-pass-1",
    "sub": "cn=ops-admin,ou=users,dc=example,dc=local",
    "email": "ops-admin@example.local"
  }
]
"#;
        let err = authenticate_ldap_dev_user(payload, "ops-admin", "wrong-pass")
            .expect_err("invalid password must fail");
        match err {
            AppError::Forbidden(message) => {
                assert!(message.contains("invalid credentials"));
            }
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn rejects_invalid_ldap_dev_user_payload() {
        let err = authenticate_ldap_dev_user("{}", "ops-admin", "dev-pass-1")
            .expect_err("invalid json must fail");
        match err {
            AppError::Validation(message) => {
                assert!(message.contains("AUTH_LDAP_DEV_USERS_JSON"));
            }
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn derives_username_from_priority_order() {
        let picked = choose_base_username(Some("Ops-Admin"), Some("x@y.z"), "fallback")
            .expect("preferred should be used");
        assert_eq!(picked, "ops-admin");

        let picked = choose_base_username(None, Some("User.Name@example.local"), "fallback")
            .expect("secondary should be used");
        assert_eq!(picked, "user.nameexample.local");
    }

    #[test]
    fn parses_ldap_group_role_mapping_and_resolves_groups() {
        let mapping_json = r#"{
  "ops-admins": ["admin"],
  "oncall": ["operator", "viewer"]
}"#;
        let mapping = parse_ldap_group_role_mapping(Some(mapping_json)).expect("must parse");
        assert_eq!(mapping.get("ops-admins"), Some(&vec!["admin".to_string()]));

        let resolved =
            resolve_ldap_role_keys(&mapping, &["ops-admins".to_string(), "oncall".to_string()]);
        assert_eq!(
            resolved,
            vec![
                "admin".to_string(),
                "operator".to_string(),
                "viewer".to_string()
            ]
        );
    }

    #[test]
    fn rejects_invalid_ldap_group_role_mapping_shape() {
        let err =
            parse_ldap_group_role_mapping(Some("[]")).expect_err("non-object mapping must fail");
        match err {
            AppError::Validation(message) => {
                assert!(message.contains("JSON object"));
            }
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn resolves_empty_roles_when_no_group_matches() {
        let mut mapping = BTreeMap::new();
        mapping.insert("ops-admins".to_string(), vec!["admin".to_string()]);
        let resolved = resolve_ldap_role_keys(&mapping, &["unknown".to_string()]);
        assert!(resolved.is_empty());
    }

    #[test]
    fn verifies_totp_code_for_current_time_window() {
        let secret = "dev-secret-123";
        let now = Utc
            .with_ymd_and_hms(2026, 3, 5, 12, 0, 0)
            .single()
            .expect("valid datetime");
        let counter = (now.timestamp() / 30) as u64;
        let code = compute_totp_code_for_counter(secret.as_bytes(), counter);

        assert!(verify_totp_code(secret, code.as_str(), now));
        assert!(!verify_totp_code(secret, "000000", now));
    }

    #[test]
    fn computes_local_session_revoke_count_edges() {
        assert_eq!(local_session_revoke_count(0, 3), 0);
        assert_eq!(local_session_revoke_count(2, 3), 0);
        assert_eq!(local_session_revoke_count(3, 3), 1);
        assert_eq!(local_session_revoke_count(5, 3), 3);
        assert_eq!(local_session_revoke_count(2, 0), 2);
    }

    #[test]
    fn validates_live_ldap_config_accepts_safe_settings() {
        let settings = sample_live_ldap_settings();
        let result = validate_ldap_live_config(&settings);
        assert!(result.is_ok());
    }

    #[test]
    fn validates_live_ldap_config_rejects_missing_url() {
        let mut settings = sample_live_ldap_settings();
        settings.live_url = None;
        let err =
            validate_ldap_live_config(&settings).expect_err("missing ldap live url should fail");
        assert_eq!(err.to_string(), "AUTH_LDAP_LIVE_URL is required");
    }

    #[test]
    fn validates_live_ldap_config_rejects_insecure_tls_skip_verify() {
        let mut settings = sample_live_ldap_settings();
        settings.live_tls_insecure_skip_verify = true;
        let err =
            validate_ldap_live_config(&settings).expect_err("insecure tls skip verify should fail");
        assert!(err.to_string().contains("not allowed in live mode"));
    }

    #[test]
    fn validates_live_ldap_config_rejects_filter_without_placeholder() {
        let mut settings = sample_live_ldap_settings();
        settings.live_user_filter = "(uid=ops-admin)".to_string();
        let err = validate_ldap_live_config(&settings)
            .expect_err("missing username placeholder should fail");
        assert!(err.to_string().contains("must include '{username}'"));
    }

    #[test]
    fn builds_live_ldap_user_filter_by_substituting_username() {
        let filter = build_ldap_live_user_filter("(uid={username})", "ops\\2aadmin")
            .expect("filter should build");
        assert_eq!(filter, "(uid=ops\\2aadmin)");
    }

    #[test]
    fn normalizes_ldap_group_value_from_dn_or_plain_value() {
        let from_dn = normalize_ldap_group_value("CN=Ops-Admins,OU=Groups,DC=example,DC=local");
        assert_eq!(from_dn, "ops-admins");

        let plain = normalize_ldap_group_value("platform-oncall");
        assert_eq!(plain, "platform-oncall");
    }

    #[test]
    fn verifies_argon2_local_password() {
        let (salt, hash) = hash_local_password_argon2("ChangeMe_12345").expect("hash must work");
        let credential = LocalCredentialRow {
            user_id: 1,
            password_salt: salt,
            password_hash: hash,
            mfa_enabled: false,
            totp_secret: None,
            failed_attempts: 0,
            last_failed_at: None,
            locked_until: None,
        };

        let ok = verify_local_password(&credential, "ChangeMe_12345").expect("verify should work");
        assert!(matches!(ok, LocalPasswordCheckResult::Verified));

        let wrong = verify_local_password(&credential, "wrong-pass").expect("verify should work");
        assert!(matches!(wrong, LocalPasswordCheckResult::Invalid));
    }

    #[test]
    fn verifies_legacy_password_and_requests_migration() {
        let salt = "legacy-salt-1".to_string();
        let hash = hash_local_password_sha256_legacy(salt.as_str(), "ChangeMe_12345");
        let credential = LocalCredentialRow {
            user_id: 1,
            password_salt: salt,
            password_hash: hash,
            mfa_enabled: false,
            totp_secret: None,
            failed_attempts: 0,
            last_failed_at: None,
            locked_until: None,
        };

        let result =
            verify_local_password(&credential, "ChangeMe_12345").expect("verify should work");
        match result {
            LocalPasswordCheckResult::VerifiedLegacyNeedsMigration { new_salt, new_hash } => {
                assert!(!new_salt.is_empty());
                assert!(new_hash.starts_with("$argon2"));
            }
            _ => panic!("legacy hash should require migration"),
        }
    }

    #[test]
    fn builds_unique_mfa_recovery_codes_with_expected_format() {
        let mut unique = BTreeSet::new();
        while unique.len() < 8 {
            unique.insert(build_local_mfa_recovery_code());
        }
        let codes: Vec<String> = unique.into_iter().collect();
        assert_eq!(codes.len(), 8);

        for code in codes {
            let parts: Vec<&str> = code.split('-').collect();
            assert_eq!(parts.len(), 3);
            for part in parts {
                assert_eq!(part.len(), 4);
                assert!(
                    part.chars()
                        .all(|ch| ch.is_ascii_digit() || ('A'..='F').contains(&ch))
                );
            }
        }
    }

    #[test]
    fn hashes_mfa_recovery_code_with_case_and_whitespace_normalization() {
        let hashed = hash_local_mfa_recovery_code(" abcd-1234-ef00 ");
        let normalized = hash_local_mfa_recovery_code("ABCD-1234-EF00");
        assert_eq!(hashed, normalized);
    }
}
