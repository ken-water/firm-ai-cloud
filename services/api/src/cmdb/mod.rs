mod assets;
mod discovery;
mod field_definitions;
mod relations;

pub fn routes() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .merge(discovery::routes())
        .merge(field_definitions::routes())
        .merge(relations::routes())
        .merge(assets::routes())
}
