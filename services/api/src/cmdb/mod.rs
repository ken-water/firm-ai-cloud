mod assets;

pub fn routes() -> axum::Router<crate::state::AppState> {
    axum::Router::new().merge(assets::routes())
}
