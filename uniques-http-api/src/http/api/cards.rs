mod handlers;
pub(crate) mod models;
#[cfg(test)]
pub(crate) mod test_support;

pub(crate) mod parse;

use axum::Router;

use crate::http::ServerState;

pub use models::{CardV2, CardsIter, CardsResponse};

pub fn router() -> Router<ServerState> {
    use axum::routing::get;

    Router::new()
        .route("/api/v2/cards", get(handlers::get_cards_v2))
        .route("/api/v2/card/{reference}", get(handlers::get_card_v2))
}
