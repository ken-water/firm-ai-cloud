mod assets;
mod field_definitions;
mod relations;

pub fn routes() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .merge(field_definitions::routes())
        .merge(relations::routes())
        .merge(assets::routes())
}
