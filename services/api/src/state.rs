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
pub struct WorkflowExecutionSettings {
    pub policy_mode: String,
    pub allowlist: Vec<String>,
    pub sandbox_dir: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub rbac_enabled: bool,
    pub oidc: OidcSettings,
    pub workflow_execution: WorkflowExecutionSettings,
}
