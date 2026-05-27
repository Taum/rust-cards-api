use crate::bitmap::BitmapStore;
use crate::card::{IdGdOccurrence, LocaleText};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct IdGdCatalog {
    pub set: String,
    pub entries: Vec<IdGdCatalogEntry>,
}

#[derive(Debug, Serialize)]
pub struct IdGdCatalogEntry {
    pub id_gd: u32,
    pub card_count: u64,
    pub bitmap_bytes: u64,
    pub bitmap_file: String,
    pub element_type: String,
    pub translations: BTreeMap<String, LocaleText>,
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
    ) -> IdGdCatalog {
        let mut entries = Vec::with_capacity(bitmaps.len());
        for (&id_gd, bitmap) in bitmaps.iter() {
            let draft = self.entries.get(&id_gd);
            let (element_type, translations) = match draft {
                Some(d) => (d.element_type.clone(), d.translations.clone()),
                None => ("UNKNOWN".to_string(), BTreeMap::new()),
            };
            entries.push(IdGdCatalogEntry {
                id_gd,
                card_count: bitmap.len(),
                bitmap_bytes: bitmap_bytes.get(&id_gd).copied().unwrap_or(0),
                bitmap_file: format!("{id_gd}.roar"),
                element_type,
                translations,
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
