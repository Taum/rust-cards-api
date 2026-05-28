use std::sync::Arc;

use axum::{Json, Router, routing::get};
use serde::Serialize;

mod env;

pub mod loader;
pub mod state;

pub use env::load_env;

pub use loader::load_index;
pub use state::AppState;

#[derive(Serialize)]
pub struct HelloResponse {
    pub message: &'static str,
}

pub async fn health() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello World",
    })
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new().route("/health", get(health)).with_state(state)
}
