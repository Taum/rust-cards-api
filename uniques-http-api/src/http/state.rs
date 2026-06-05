use std::sync::{Arc, RwLock};

use crate::index::UniquesIndex;

/// Axum shared state; holds the loaded index and will gain HTTP-layer fields over time.
pub struct AppState {
    index: RwLock<Arc<UniquesIndex>>,
}

impl AppState {
    pub(crate) fn new(index: UniquesIndex) -> Self {
        Self {
            index: RwLock::new(Arc::new(index)),
        }
    }

    pub fn index(&self) -> Arc<UniquesIndex> {
        self.index
            .read()
            .expect("index lock poisoned")
            .clone()
    }

    pub fn current_built_at_secs(&self) -> u64 {
        self.index
            .read()
            .expect("index lock poisoned")
            .manifest
            .built_at_secs
    }

    /// Swap only if `new` has a strictly higher `built_at_secs`.
    /// Returns `Some((old_secs, new_secs))` when swapped, `None` if skipped.
    pub(crate) fn swap_if_newer(&self, new: Arc<UniquesIndex>) -> Option<(u64, u64)> {
        let mut guard = self.index.write().expect("index lock poisoned");
        let old_secs = guard.manifest.built_at_secs;
        let new_secs = new.manifest.built_at_secs;
        if new_secs > old_secs {
            *guard = new;
            Some((old_secs, new_secs))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use axum::body::Bytes;

    use alt_indexer::catalog::Catalog;
    use alt_indexer::idgd_catalog::IdGdCatalog;

    use crate::index::loader::{
        build_family_lookup_index, build_name_search_index, FactionsSummary, IndexManifest,
        StatsSummary,
    };
    use crate::index::UniquesIndex;

    use super::AppState;

    fn minimal_index(built_at_secs: u64) -> UniquesIndex {
        let catalog = Catalog {
            set: "TEST".to_string(),
            faction_order: vec![],
            families: vec![],
            total_bit_span: 0,
        };
        UniquesIndex {
            index_dir: PathBuf::from("/test"),
            catalog: catalog.clone(),
            manifest: IndexManifest {
                version: 1,
                set: "TEST".to_string(),
                kind: None,
                built_at_secs,
                card_count: 0,
                id_gd_count: 0,
                total_bit_span: 0,
                family_count: 0,
            },
            idgd_catalog: IdGdCatalog {
                set: "TEST".to_string(),
                entries: vec![],
            },
            stats_summary: StatsSummary {
                version: 1,
                set: "TEST".to_string(),
                total_cards_indexed: 0,
                fields: vec![],
            },
            factions_summary: FactionsSummary {
                version: 1,
                set: "TEST".to_string(),
                total_cards_indexed: 0,
                source: String::new(),
                factions: vec![],
                unknown_count: 0,
                bitmap_dir: String::new(),
            },
            cards: vec![],
            id_gd_whole: Default::default(),
            id_gd_per_line: Default::default(),
            stats: Default::default(),
            factions: Default::default(),
            set_bitmaps: crate::index::loader::SetBitmaps {
                by_set: Default::default(),
                core_and_coreks: None,
            },
            name_search_index: build_name_search_index(&catalog),
            family_lookup_index: build_family_lookup_index(&catalog),
            family_span_groups: vec![],
            effects_body: Arc::new(Bytes::from_static(b"[]")),
        }
    }

    #[test]
    fn swap_if_newer_replaces_when_strictly_newer() {
        let state = AppState::new(minimal_index(10));
        let swapped = state.swap_if_newer(Arc::new(minimal_index(20)));
        assert_eq!(swapped, Some((10, 20)));
        assert_eq!(state.current_built_at_secs(), 20);
    }

    #[test]
    fn swap_if_newer_skips_when_equal_or_older() {
        let state = AppState::new(minimal_index(20));
        assert_eq!(state.swap_if_newer(Arc::new(minimal_index(20))), None);
        assert_eq!(state.swap_if_newer(Arc::new(minimal_index(10))), None);
        assert_eq!(state.current_built_at_secs(), 20);
    }
}
