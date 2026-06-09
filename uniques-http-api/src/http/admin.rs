mod handlers;

use axum::{routing::get, Router};

use crate::http::ServerState;

pub fn router() -> Router<ServerState> {
    Router::new().route("/healthz", get(handlers::healthz))
}
