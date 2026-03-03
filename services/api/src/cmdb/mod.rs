mod assets;
mod discovery;
mod field_definitions;
mod lifecycle;
mod notifications;
mod relations;

pub fn routes() -> axum::Router<crate::state::AppState> {
    axum::Router::new()
        .merge(discovery::routes())
        .merge(field_definitions::routes())
        .merge(lifecycle::routes())
        .merge(notifications::routes())
        .merge(relations::routes())
        .merge(assets::routes())
}
