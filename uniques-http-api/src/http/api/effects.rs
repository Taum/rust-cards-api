mod filtered;
mod handlers;
mod list;
mod models;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::http::state::AppState;

pub use list::{build_effects_list, serialize_effects_list};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v2/effects", get(handlers::get_effects_v2))
        .route(
            "/api/v2/effects/filtered",
            get(handlers::get_effects_filtered),
        )
}
