pub mod cards;
pub(crate) mod error;
pub mod effects;

use axum::Router;

use crate::http::ServerState;

pub fn router() -> Router<ServerState> {
    Router::new()
        .merge(cards::router())
        .merge(effects::router())
}
