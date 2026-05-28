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
    /// `false` = MAIN_EFFECT only, `true` = ECHO_EFFECT only, `null` = seen in both (build error).
    pub is_echo: Option<bool>,
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
    seen_main: bool,
    seen_echo: bool,
}

/// Derive catalog `is_echo` from which effect regions referenced this idGd during build.
pub fn is_echo_from_flags(seen_main: bool, seen_echo: bool, id_gd: u32) -> Option<bool> {
    match (seen_main, seen_echo) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        (true, true) => {
            eprintln!("error: idGd {id_gd} appears in both MAIN_EFFECT and ECHO_EFFECT");
            None
        }
        (false, false) => None,
    }
}

/// Combine `is_echo` values when merging per-set catalogs.
pub fn merge_is_echo_values(values: &[Option<bool>], id_gd: u32) -> Option<bool> {
    let mut saw_null = false;
    let mut saw_true = false;
    let mut saw_false = false;
    for v in values {
        match v {
            None => saw_null = true,
            Some(true) => saw_true = true,
            Some(false) => saw_false = true,
        }
    }
    if saw_true && saw_false {
        eprintln!(
            "error: idGd {id_gd} has conflicting is_echo across merged source catalogs (main vs echo)"
        );
        return None;
    }
    if saw_null && (saw_true || saw_false) {
        eprintln!(
            "error: idGd {id_gd} has is_echo=null in one source catalog but a definite value in another"
        );
        return None;
    }
    if saw_true {
        Some(true)
    } else if saw_false {
        Some(false)
    } else {
        None
    }
}

impl IdGdCatalogBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record metadata from the first time this `idGd` is seen across the build.
    pub fn record_first(&mut self, occurrence: &IdGdOccurrence) {
        self.entries
            .entry(occurrence.id_gd)
            .or_insert_with(|| DraftEntry {
                element_type: occurrence.element_type.clone(),
                translations: occurrence.translations.clone(),
                seen_main: false,
                seen_echo: false,
            });
    }

    /// Record that `id_gd` appeared on an effect line (`m1`..`m3` = main, `ec` = echo).
    pub fn record_effect_line(&mut self, id_gd: u32, line: EffectLine) {
        let entry = self.entries.entry(id_gd).or_insert_with(|| DraftEntry {
            element_type: "UNKNOWN".to_string(),
            translations: BTreeMap::new(),
            seen_main: false,
            seen_echo: false,
        });
        match line {
            EffectLine::Ec => entry.seen_echo = true,
            _ => entry.seen_main = true,
        }
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
            let (element_type, translations, is_echo) = match draft {
                Some(d) => (
                    d.element_type.clone(),
                    d.translations.clone(),
                    is_echo_from_flags(d.seen_main, d.seen_echo, id_gd),
                ),
                None => ("UNKNOWN".to_string(), BTreeMap::new(), None),
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
                is_echo,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_echo_from_flags_main_only() {
        assert_eq!(is_echo_from_flags(true, false, 1), Some(false));
    }

    #[test]
    fn is_echo_from_flags_echo_only() {
        assert_eq!(is_echo_from_flags(false, true, 2), Some(true));
    }

    #[test]
    fn is_echo_from_flags_both_is_null() {
        assert_eq!(is_echo_from_flags(true, true, 3), None);
    }
}
