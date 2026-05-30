use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use alt_indexer::bitmap::EffectLine;
use alt_indexer::catalog::Catalog;
use alt_indexer::compact::{CompactCardView, RECORD_SIZE};
use alt_indexer::faction_index::Faction;
use alt_indexer::idgd_catalog::IdGdCatalog;
use alt_indexer::stat_index::StatField;
use roaring::RoaringBitmap;

use alt_indexer::path::parse_card_reference;

use crate::loader::{
    FamilyLookupIndex, FamilyResolveError, FamilySpanGroup, FactionsSummary, IndexManifest,
    NameSearchIndex, SetBitmaps, StatsSummary,
};

/// Shared read-only index data, loaded once at startup and cloned per request via `Arc`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub index_dir: PathBuf,
    pub catalog: Catalog,
    pub manifest: IndexManifest,
    pub idgd_catalog: IdGdCatalog,
    pub stats_summary: StatsSummary,
    pub factions_summary: FactionsSummary,
    pub cards: Vec<u8>,
    /// Whole-card idGd bitmaps: `id_gd/{id}.roar`
    pub id_gd_whole: BTreeMap<u32, RoaringBitmap>,
    /// Per-line idGd bitmaps: `id_gd/{id}_m1.roar`, etc.
    pub id_gd_per_line: BTreeMap<(u32, EffectLine), RoaringBitmap>,
    /// Stat bucket bitmaps keyed by field and value 0..15.
    pub stats: BTreeMap<StatField, [RoaringBitmap; 16]>,
    pub factions: BTreeMap<Faction, RoaringBitmap>,
    pub set_bitmaps: SetBitmaps,
    pub name_search_index: NameSearchIndex,
    pub family_lookup_index: FamilyLookupIndex,
    pub family_span_groups: Vec<FamilySpanGroup>,
    /// Pre-serialized `GET /api/v2/effects` JSON body.
    pub effects_body: Arc<Bytes>,
}

impl AppState {
    pub(crate) fn new(inner: Arc<AppStateInner>) -> Self {
        Self { inner }
    }

    pub fn index_dir(&self) -> &PathBuf {
        &self.inner.index_dir
    }

    pub fn catalog(&self) -> &Catalog {
        &self.inner.catalog
    }

    pub fn manifest(&self) -> &IndexManifest {
        &self.inner.manifest
    }

    pub fn idgd_catalog(&self) -> &IdGdCatalog {
        &self.inner.idgd_catalog
    }

    pub fn stats_summary(&self) -> &StatsSummary {
        &self.inner.stats_summary
    }

    pub fn factions_summary(&self) -> &FactionsSummary {
        &self.inner.factions_summary
    }

    pub fn cards(&self) -> &[u8] {
        &self.inner.cards
    }

    pub fn id_gd_whole(&self) -> &BTreeMap<u32, RoaringBitmap> {
        &self.inner.id_gd_whole
    }

    pub fn id_gd_per_line(&self) -> &BTreeMap<(u32, EffectLine), RoaringBitmap> {
        &self.inner.id_gd_per_line
    }

    pub fn stats(&self) -> &BTreeMap<StatField, [RoaringBitmap; 16]> {
        &self.inner.stats
    }

    pub fn factions(&self) -> &BTreeMap<Faction, RoaringBitmap> {
        &self.inner.factions
    }

    pub fn set_bitmaps(&self) -> &SetBitmaps {
        &self.inner.set_bitmaps
    }

    pub fn name_search_index(&self) -> &NameSearchIndex {
        &self.inner.name_search_index
    }

    pub fn family_lookup_index(&self) -> &FamilyLookupIndex {
        &self.inner.family_lookup_index
    }

    pub fn family_span_groups(&self) -> &[FamilySpanGroup] {
        &self.inner.family_span_groups
    }

    pub fn effects_body(&self) -> &Arc<Bytes> {
        &self.inner.effects_body
    }

    pub fn card_view(&self, card_index: u32) -> Option<CompactCardView<'_>> {
        CompactCardView::from_data(&self.inner.cards, card_index)
    }

    pub fn decode_reference(&self, card_index: u32) -> anyhow::Result<String> {
        Ok(self.inner.catalog.decode_bit(card_index)?.reference)
    }

    pub fn resolve_card_index(&self, reference: &str) -> Result<u32, CardResolveError> {
        let parsed = parse_card_reference(reference).map_err(|e| CardResolveError::BadRequest {
            message: e.to_string(),
        })?;
        self.inner
            .family_lookup_index
            .resolve(&parsed)
            .map_err(|e| match e {
                FamilyResolveError::NotFound => CardResolveError::NotFound {
                    message: format!("reference not found: {}", parsed.reference()),
                },
                FamilyResolveError::Padding => CardResolveError::NotFound {
                    message: format!(
                        "reference {} falls in padding (max UniqueID {})",
                        parsed.reference(),
                        self.inner
                            .family_lookup_index
                            .max_unique_id(&parsed)
                            .unwrap_or(0)
                    ),
                },
            })
    }
}

#[derive(Debug)]
pub enum CardResolveError {
    BadRequest { message: String },
    NotFound { message: String },
}

impl AppStateInner {
    pub fn expected_cards_len(total_bit_span: u32) -> u64 {
        total_bit_span as u64 * RECORD_SIZE as u64
    }
}
