pub(crate) mod admin;
pub mod api;
pub mod extract;
pub mod state;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;

pub use extract::IndexSnapshot;
pub use state::AppState;

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(admin::router())
        .merge(api::router())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
