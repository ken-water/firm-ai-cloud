mod assets;
mod field_definitions;

pub fn routes() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .merge(field_definitions::routes())
        .merge(assets::routes())
}
