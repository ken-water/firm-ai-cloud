#[derive(Clone)]
pub struct OidcSettings {
    pub enabled: bool,
    pub authorization_endpoint: Option<String>,
    pub token_endpoint: Option<String>,
    pub userinfo_endpoint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub scope: String,
    pub auto_provision: bool,
    pub session_ttl_minutes: u32,
    pub dev_mode_enabled: bool,
}

#[derive(Clone)]
pub struct LdapSettings {
    pub enabled: bool,
    pub mode: String,
    pub auto_provision: bool,
    pub dev_users_json: Option<String>,
    pub group_role_mapping_json: Option<String>,
}

#[derive(Clone)]
pub struct LocalAuthSettings {
    pub fallback_mode: String,
    pub break_glass_users: Vec<String>,
}

#[derive(Clone)]
pub struct WorkflowExecutionSettings {
    pub policy_mode: String,
    pub allowlist: Vec<String>,
    pub sandbox_dir: String,
}

#[derive(Clone)]
pub struct MonitoringSecretSettings {
    pub encryption_key: Option<String>,
    pub inline_policy: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub rbac_enabled: bool,
    pub oidc: OidcSettings,
    pub ldap: LdapSettings,
    pub local_auth: LocalAuthSettings,
    pub monitoring_secret: MonitoringSecretSettings,
    pub workflow_execution: WorkflowExecutionSettings,
}
