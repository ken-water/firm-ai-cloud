#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub rbac_enabled: bool,
}
