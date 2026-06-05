mod handlers;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::http::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/healthz", get(handlers::healthz))
}
