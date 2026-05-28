use crate::bitmap::{BitmapStore, EffectLine, PerLineBitmapStore};
use crate::card::{IdGdOccurrence, LocaleText};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct IdGdCatalog {
    pub set: String,
    pub entries: Vec<IdGdCatalogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdGdCatalogEntry {
    pub id_gd: u32,
    pub card_count: u64,
    pub bitmap_bytes: u64,
    pub bitmap_file: String,
    pub element_type: String,
    pub translations: BTreeMap<String, LocaleText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m1: Option<BitmapMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m2: Option<BitmapMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m3: Option<BitmapMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ec: Option<BitmapMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitmapMeta {
    pub card_count: u64,
    pub bitmap_bytes: u64,
    pub bitmap_file: String,
}

#[derive(Debug, Default)]
pub struct IdGdCatalogBuilder {
    entries: BTreeMap<u32, DraftEntry>,
}

#[derive(Debug, Clone)]
struct DraftEntry {
    element_type: String,
    translations: BTreeMap<String, LocaleText>,
}

impl IdGdCatalogBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record metadata from the first time this `idGd` is seen across the build.
    pub fn record_first(&mut self, occurrence: &IdGdOccurrence) {
        self.entries.entry(occurrence.id_gd).or_insert_with(|| DraftEntry {
            element_type: occurrence.element_type.clone(),
            translations: occurrence.translations.clone(),
        });
    }

    pub fn build(
        self,
        set: &str,
        bitmaps: &BitmapStore,
        bitmap_bytes: &BTreeMap<u32, u64>,
        per_line_bitmaps: &PerLineBitmapStore,
        per_line_bitmap_bytes: &BTreeMap<(u32, EffectLine), u64>,
    ) -> IdGdCatalog {
        let mut entries = Vec::with_capacity(bitmaps.len());
        for (&id_gd, bitmap) in bitmaps.iter() {
            let draft = self.entries.get(&id_gd);
            let (element_type, translations) = match draft {
                Some(d) => (d.element_type.clone(), d.translations.clone()),
                None => ("UNKNOWN".to_string(), BTreeMap::new()),
            };

            let line_meta = |line: EffectLine| -> Option<BitmapMeta> {
                let bmp = per_line_bitmaps.get(id_gd, line)?;
                if bmp.is_empty() {
                    return None;
                }
                let bytes = per_line_bitmap_bytes
                    .get(&(id_gd, line))
                    .copied()
                    .unwrap_or(0);
                Some(BitmapMeta {
                    card_count: bmp.len(),
                    bitmap_bytes: bytes,
                    bitmap_file: format!("{id_gd}_{}.roar", line.suffix()),
                })
            };

            entries.push(IdGdCatalogEntry {
                id_gd,
                card_count: bitmap.len(),
                bitmap_bytes: bitmap_bytes.get(&id_gd).copied().unwrap_or(0),
                bitmap_file: format!("{id_gd}.roar"),
                element_type,
                translations,
                m1: line_meta(EffectLine::M1),
                m2: line_meta(EffectLine::M2),
                m3: line_meta(EffectLine::M3),
                ec: line_meta(EffectLine::Ec),
            });
        }
        IdGdCatalog {
            set: set.to_string(),
            entries,
        }
    }

    pub fn save(catalog: &IdGdCatalog, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(catalog)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
