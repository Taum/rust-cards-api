pub mod cards;
pub(crate) mod error;
pub mod effects;

use std::sync::Arc;

use axum::Router;

use crate::http::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(cards::router())
        .merge(effects::router())
}
