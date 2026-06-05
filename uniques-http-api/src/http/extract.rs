use std::convert::Infallible;
use std::ops::Deref;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::http::state::AppState;
use crate::index::UniquesIndex;

/// Per-request snapshot of the current index (one `Arc` clone, held for the handler lifetime).
pub struct IndexSnapshot(pub Arc<UniquesIndex>);

impl Deref for IndexSnapshot {
    type Target = UniquesIndex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequestParts<Arc<AppState>> for IndexSnapshot {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        Ok(IndexSnapshot(state.index()))
    }
}
