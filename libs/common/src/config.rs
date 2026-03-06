use std::env;
use std::num::ParseIntError;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub database_url: String,
    pub db_max_connections: u32,
    pub rbac_enabled: bool,
    pub oidc_enabled: bool,
    pub oidc_authorization_endpoint: Option<String>,
    pub oidc_token_endpoint: Option<String>,
    pub oidc_userinfo_endpoint: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    pub oidc_redirect_uri: Option<String>,
    pub oidc_scope: String,
    pub oidc_auto_provision: bool,
    pub oidc_session_ttl_minutes: u32,
    pub oidc_dev_mode_enabled: bool,
    pub ldap_enabled: bool,
    pub ldap_mode: String,
    pub ldap_auto_provision: bool,
    pub ldap_dev_users_json: Option<String>,
    pub ldap_group_role_mapping_json: Option<String>,
    pub ldap_live_url: Option<String>,
    pub ldap_live_bind_dn: Option<String>,
    pub ldap_live_bind_password: Option<String>,
    pub ldap_live_base_dn: Option<String>,
    pub ldap_live_user_filter: String,
    pub ldap_live_attr_email: String,
    pub ldap_live_attr_display_name: String,
    pub ldap_live_attr_groups: String,
    pub ldap_live_starttls: bool,
    pub ldap_live_tls_insecure_skip_verify: bool,
    pub local_fallback_mode: String,
    pub local_break_glass_users: Vec<String>,
    pub local_session_idle_timeout_minutes: u32,
    pub local_session_max_age_minutes: u32,
    pub local_session_max_concurrent: u32,
    pub local_lockout_threshold: u32,
    pub local_lockout_minutes: u32,
    pub local_rate_limit_window_seconds: u32,
    pub local_rate_limit_max_attempts: u32,
    pub monitoring_secret_encryption_key: Option<String>,
    pub monitoring_secret_inline_policy: String,
    pub workflow_execution_policy_mode: String,
    pub workflow_execution_allowlist: Vec<String>,
    pub workflow_execution_sandbox_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = parse_u16_env("API_PORT", 8080)?;
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://cloudops:cloudops_local_change_me@127.0.0.1:5432/cloudops".to_string()
        });
        let db_max_connections = parse_u32_env("DB_MAX_CONNECTIONS", 10)?;
        let rbac_enabled = parse_bool_env("AUTH_RBAC_ENABLED", true)?;
        let oidc_enabled = parse_bool_env("AUTH_OIDC_ENABLED", false)?;
        let oidc_authorization_endpoint = parse_optional_env("AUTH_OIDC_AUTHORIZATION_ENDPOINT");
        let oidc_token_endpoint = parse_optional_env("AUTH_OIDC_TOKEN_ENDPOINT");
        let oidc_userinfo_endpoint = parse_optional_env("AUTH_OIDC_USERINFO_ENDPOINT");
        let oidc_client_id = parse_optional_env("AUTH_OIDC_CLIENT_ID");
        let oidc_client_secret = parse_optional_env("AUTH_OIDC_CLIENT_SECRET");
        let oidc_redirect_uri = parse_optional_env("AUTH_OIDC_REDIRECT_URI");
        let oidc_scope =
            env::var("AUTH_OIDC_SCOPE").unwrap_or_else(|_| "openid profile email".to_string());
        let oidc_auto_provision = parse_bool_env("AUTH_OIDC_AUTO_PROVISION", false)?;
        let oidc_session_ttl_minutes = parse_u32_env("AUTH_SESSION_TTL_MINUTES", 480)?;
        let oidc_dev_mode_enabled = parse_bool_env("AUTH_OIDC_DEV_MODE_ENABLED", false)?;
        let ldap_enabled = parse_bool_env("AUTH_LDAP_ENABLED", false)?;
        let ldap_mode = parse_enum_env("AUTH_LDAP_MODE", "dev", &["dev", "live"])?;
        let ldap_auto_provision = parse_bool_env("AUTH_LDAP_AUTO_PROVISION", false)?;
        let ldap_dev_users_json = parse_optional_env("AUTH_LDAP_DEV_USERS_JSON");
        let ldap_group_role_mapping_json = parse_optional_env("AUTH_LDAP_GROUP_ROLE_MAPPING_JSON");
        let ldap_live_url = parse_optional_env("AUTH_LDAP_LIVE_URL");
        let ldap_live_bind_dn = parse_optional_env("AUTH_LDAP_LIVE_BIND_DN");
        let ldap_live_bind_password = parse_optional_env("AUTH_LDAP_LIVE_BIND_PASSWORD");
        let ldap_live_base_dn = parse_optional_env("AUTH_LDAP_LIVE_BASE_DN");
        let ldap_live_user_filter = env::var("AUTH_LDAP_LIVE_USER_FILTER")
            .unwrap_or_else(|_| "(uid={username})".to_string());
        let ldap_live_attr_email =
            env::var("AUTH_LDAP_LIVE_ATTR_EMAIL").unwrap_or_else(|_| "mail".to_string());
        let ldap_live_attr_display_name = env::var("AUTH_LDAP_LIVE_ATTR_DISPLAY_NAME")
            .unwrap_or_else(|_| "displayName".to_string());
        let ldap_live_attr_groups =
            env::var("AUTH_LDAP_LIVE_ATTR_GROUPS").unwrap_or_else(|_| "memberOf".to_string());
        let ldap_live_starttls = parse_bool_env("AUTH_LDAP_LIVE_STARTTLS", false)?;
        let ldap_live_tls_insecure_skip_verify =
            parse_bool_env("AUTH_LDAP_LIVE_TLS_INSECURE_SKIP_VERIFY", false)?;
        let local_fallback_mode = parse_enum_env(
            "AUTH_LOCAL_FALLBACK_MODE",
            "allow_all",
            &["allow_all", "break_glass_only", "disabled"],
        )?;
        let local_break_glass_users = parse_csv_env("AUTH_LOCAL_BREAK_GLASS_USERS");
        let local_session_idle_timeout_minutes =
            parse_u32_env("AUTH_LOCAL_SESSION_IDLE_TIMEOUT_MINUTES", 60)?;
        let local_session_max_age_minutes =
            parse_u32_env("AUTH_LOCAL_SESSION_MAX_AGE_MINUTES", 480)?;
        let local_session_max_concurrent = parse_u32_env("AUTH_LOCAL_SESSION_MAX_CONCURRENT", 3)?;
        let local_lockout_threshold = parse_u32_env("AUTH_LOCAL_LOCKOUT_THRESHOLD", 5)?;
        let local_lockout_minutes = parse_u32_env("AUTH_LOCAL_LOCKOUT_MINUTES", 15)?;
        let local_rate_limit_window_seconds =
            parse_u32_env("AUTH_LOCAL_RATE_LIMIT_WINDOW_SECONDS", 60)?;
        let local_rate_limit_max_attempts =
            parse_u32_env("AUTH_LOCAL_RATE_LIMIT_MAX_ATTEMPTS", 10)?;
        let monitoring_secret_encryption_key =
            parse_optional_env("MONITORING_SECRET_ENCRYPTION_KEY");
        let monitoring_secret_inline_policy = parse_enum_env(
            "MONITORING_SECRET_INLINE_POLICY",
            "allow",
            &["allow", "forbid"],
        )?;
        let workflow_execution_policy_mode = parse_enum_env(
            "WORKFLOW_EXECUTION_POLICY_MODE",
            "disabled",
            &["disabled", "allowlist", "sandboxed"],
        )?;
        let workflow_execution_allowlist = parse_csv_env("WORKFLOW_EXECUTION_ALLOWLIST");
        let workflow_execution_sandbox_dir = env::var("WORKFLOW_EXECUTION_SANDBOX_DIR")
            .unwrap_or_else(|_| "/tmp/cloudops-workflow-sandbox".to_string());

        Ok(Self {
            host,
            port,
            log_level,
            database_url,
            db_max_connections,
            rbac_enabled,
            oidc_enabled,
            oidc_authorization_endpoint,
            oidc_token_endpoint,
            oidc_userinfo_endpoint,
            oidc_client_id,
            oidc_client_secret,
            oidc_redirect_uri,
            oidc_scope,
            oidc_auto_provision,
            oidc_session_ttl_minutes,
            oidc_dev_mode_enabled,
            ldap_enabled,
            ldap_mode,
            ldap_auto_provision,
            ldap_dev_users_json,
            ldap_group_role_mapping_json,
            ldap_live_url,
            ldap_live_bind_dn,
            ldap_live_bind_password,
            ldap_live_base_dn,
            ldap_live_user_filter,
            ldap_live_attr_email,
            ldap_live_attr_display_name,
            ldap_live_attr_groups,
            ldap_live_starttls,
            ldap_live_tls_insecure_skip_verify,
            local_fallback_mode,
            local_break_glass_users,
            local_session_idle_timeout_minutes,
            local_session_max_age_minutes,
            local_session_max_concurrent,
            local_lockout_threshold,
            local_lockout_minutes,
            local_rate_limit_window_seconds,
            local_rate_limit_max_attempts,
            monitoring_secret_encryption_key,
            monitoring_secret_inline_policy,
            workflow_execution_policy_mode,
            workflow_execution_allowlist,
            workflow_execution_sandbox_dir,
        })
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn parse_u16_env(key: &str, default: u16) -> Result<u16, ConfigError> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|source| ConfigError::InvalidNumber {
                key: key.to_string(),
                value,
                source,
            }),
        Err(_) => Ok(default),
    }
}

fn parse_u32_env(key: &str, default: u32) -> Result<u32, ConfigError> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u32>()
            .map_err(|source| ConfigError::InvalidNumber {
                key: key.to_string(),
                value,
                source,
            }),
        Err(_) => Ok(default),
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, ConfigError> {
    match env::var(key) {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => Err(ConfigError::InvalidBool {
                    key: key.to_string(),
                    value,
                }),
            }
        }
        Err(_) => Ok(default),
    }
}

fn parse_optional_env(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_csv_env(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_enum_env(key: &str, default: &str, supported: &[&str]) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            if supported.contains(&normalized.as_str()) {
                Ok(normalized)
            } else {
                Err(ConfigError::InvalidEnum {
                    key: key.to_string(),
                    value,
                    supported: supported.iter().map(|item| item.to_string()).collect(),
                })
            }
        }
        Err(_) => Ok(default.to_string()),
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid number for {key}: {value}")]
    InvalidNumber {
        key: String,
        value: String,
        source: ParseIntError,
    },
    #[error("invalid boolean for {key}: {value}")]
    InvalidBool { key: String, value: String },
    #[error("invalid value for {key}: {value}, supported: {supported:?}")]
    InvalidEnum {
        key: String,
        value: String,
        supported: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_defaults() {
        let cfg = AppConfig::from_env().expect("default config should load");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.db_max_connections, 10);
        assert!(cfg.rbac_enabled);
        assert!(!cfg.oidc_enabled);
        assert_eq!(cfg.oidc_scope, "openid profile email");
        assert_eq!(cfg.oidc_session_ttl_minutes, 480);
        assert!(!cfg.oidc_auto_provision);
        assert!(!cfg.oidc_dev_mode_enabled);
        assert!(cfg.oidc_authorization_endpoint.is_none());
        assert!(cfg.oidc_token_endpoint.is_none());
        assert!(cfg.oidc_userinfo_endpoint.is_none());
        assert!(cfg.oidc_client_id.is_none());
        assert!(cfg.oidc_client_secret.is_none());
        assert!(cfg.oidc_redirect_uri.is_none());
        assert!(!cfg.ldap_enabled);
        assert_eq!(cfg.ldap_mode, "dev");
        assert!(!cfg.ldap_auto_provision);
        assert!(cfg.ldap_dev_users_json.is_none());
        assert!(cfg.ldap_group_role_mapping_json.is_none());
        assert!(cfg.ldap_live_url.is_none());
        assert!(cfg.ldap_live_bind_dn.is_none());
        assert!(cfg.ldap_live_bind_password.is_none());
        assert!(cfg.ldap_live_base_dn.is_none());
        assert_eq!(cfg.ldap_live_user_filter, "(uid={username})");
        assert_eq!(cfg.ldap_live_attr_email, "mail");
        assert_eq!(cfg.ldap_live_attr_display_name, "displayName");
        assert_eq!(cfg.ldap_live_attr_groups, "memberOf");
        assert!(!cfg.ldap_live_starttls);
        assert!(!cfg.ldap_live_tls_insecure_skip_verify);
        assert_eq!(cfg.local_fallback_mode, "allow_all");
        assert!(cfg.local_break_glass_users.is_empty());
        assert_eq!(cfg.local_session_idle_timeout_minutes, 60);
        assert_eq!(cfg.local_session_max_age_minutes, 480);
        assert_eq!(cfg.local_session_max_concurrent, 3);
        assert_eq!(cfg.local_lockout_threshold, 5);
        assert_eq!(cfg.local_lockout_minutes, 15);
        assert_eq!(cfg.local_rate_limit_window_seconds, 60);
        assert_eq!(cfg.local_rate_limit_max_attempts, 10);
        assert!(cfg.monitoring_secret_encryption_key.is_none());
        assert_eq!(cfg.monitoring_secret_inline_policy, "allow");
        assert_eq!(cfg.workflow_execution_policy_mode, "disabled");
        assert!(cfg.workflow_execution_allowlist.is_empty());
        assert_eq!(
            cfg.workflow_execution_sandbox_dir,
            "/tmp/cloudops-workflow-sandbox"
        );
        assert_eq!(
            cfg.database_url,
            "postgres://cloudops:cloudops_local_change_me@127.0.0.1:5432/cloudops"
        );
    }
}
