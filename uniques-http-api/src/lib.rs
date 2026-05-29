use std::sync::Arc;

use axum::{Json, Router, routing::get};
use serde::Serialize;

mod cards;
mod effects;
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
    Router::new()
        .route("/health", get(health))
        .route("/api/v2/cards", get(cards::get_cards_v2))
        .route("/api/v2/effects", get(effects::get_effects_v2))
        .with_state(state)
}
