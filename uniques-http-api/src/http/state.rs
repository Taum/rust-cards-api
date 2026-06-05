use crate::index::UniquesIndex;

/// Axum shared state; holds the loaded index and will gain HTTP-layer fields over time.
pub struct AppState {
    pub(crate) index: UniquesIndex,
}

impl AppState {
    pub(crate) fn new(index: UniquesIndex) -> Self {
        Self { index }
    }

    pub fn index(&self) -> &UniquesIndex {
        &self.index
    }
}
