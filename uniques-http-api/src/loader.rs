use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use alt_indexer::bitmap::{BitmapStore, EffectLine};
use alt_indexer::catalog::Catalog;
use alt_indexer::compact::RECORD_SIZE;
use alt_indexer::faction_index::Faction;
use alt_indexer::idgd_catalog::IdGdCatalog;
use alt_indexer::stat_index::StatField;
use anyhow::{bail, Context, Result};
use roaring::RoaringBitmap;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

use crate::effects::{build_effects_list, serialize_effects_list};
use crate::state::{AppState, AppStateInner};

#[derive(Debug, Clone, Deserialize)]
pub struct IndexManifest {
    pub version: u32,
    pub set: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub built_at_secs: u64,
    pub card_count: u32,
    pub id_gd_count: u64,
    pub total_bit_span: u32,
    pub family_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatsSummary {
    pub version: u32,
    pub set: String,
    pub total_cards_indexed: u32,
    pub fields: Vec<StatFieldSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatFieldSummary {
    pub field: String,
    pub element_reference: String,
    pub counts: BTreeMap<u8, u64>,
    pub bitmap_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactionsSummary {
    pub version: u32,
    pub set: String,
    pub total_cards_indexed: u32,
    pub source: String,
    pub factions: Vec<FactionSummary>,
    pub unknown_count: u64,
    pub bitmap_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactionSummary {
    pub reference: String,
    pub card_count: u64,
    pub bitmap_file: String,
}

pub const SET_CORE: &str = "CORE";
pub const SET_COREKS: &str = "COREKS";

#[derive(Debug, Clone)]
pub struct SetBitmaps {
    pub by_set: BTreeMap<String, RoaringBitmap>,
    /// `CORE | COREKS` when both sets exist in `by_set`; built once at load.
    pub core_and_coreks: Option<RoaringBitmap>,
}

pub fn build_set_bitmaps(catalog: &Catalog) -> SetBitmaps {
    let mut by_set: BTreeMap<String, RoaringBitmap> = BTreeMap::new();
    for family in &catalog.families {
        let code = family
            .source_set
            .as_deref()
            .unwrap_or(&catalog.set)
            .to_string();
        let end = family.start_bit.saturating_add(family.max_unique_id);
        by_set
            .entry(code)
            .or_insert_with(RoaringBitmap::new)
            .insert_range(family.start_bit..end);
    }

    let core_and_coreks = match (by_set.get(SET_CORE), by_set.get(SET_COREKS)) {
        (Some(a), Some(b)) => Some(a.clone() | b.clone()),
        _ => None,
    };

    SetBitmaps {
        by_set,
        core_and_coreks,
    }
}

/// Lowercased locale names per catalog family row, built once at index load.
#[derive(Debug, Clone)]
pub struct NameSearchIndex {
    by_family: Vec<Vec<String>>,
}

impl NameSearchIndex {
    pub fn by_family(&self) -> &[Vec<String>] {
        &self.by_family
    }

    pub fn bitmap_for_contains(&self, catalog: &Catalog, query: &str) -> RoaringBitmap {
        let needle = normalize_name_for_search(query);
        let mut bmp = RoaringBitmap::new();
        for (family, names) in catalog.families.iter().zip(&self.by_family) {
            if names.iter().any(|name| name.contains(&needle)) {
                let end = family.start_bit.saturating_add(family.max_unique_id);
                bmp.insert_range(family.start_bit..end);
            }
        }
        bmp
    }
}

/// Lowercase and strip combining marks so `elementaire` matches `Élémentaire`.
pub fn normalize_name_for_search(name: &str) -> String {
    name.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

pub fn build_name_search_index(catalog: &Catalog) -> NameSearchIndex {
    let by_family = catalog
        .families
        .iter()
        .map(|family| {
            family
                .name
                .values()
                .map(|name| normalize_name_for_search(name))
                .collect()
        })
        .collect();
    NameSearchIndex { by_family }
}

/// Eagerly load the full merged index directory into memory.
pub fn load_index(index_dir: &Path) -> Result<AppState> {
    let index_dir = index_dir
        .canonicalize()
        .with_context(|| format!("resolve index path {}", index_dir.display()))?;

    eprintln!("loading index from {}", index_dir.display());

    let catalog = Catalog::load(&index_dir.join("catalog.json"))
        .with_context(|| "load catalog.json")?;
    eprintln!(
        "  catalog: {} families, total_bit_span={}",
        catalog.families.len(),
        catalog.total_bit_span
    );

    let manifest = load_json(&index_dir.join("manifest.json"))?;
    let idgd_catalog: IdGdCatalog = load_json(&index_dir.join("idgd_catalog.json"))?;
    let stats_summary: StatsSummary = load_json(&index_dir.join("stats_summary.json"))?;
    let factions_summary: FactionsSummary = load_json(&index_dir.join("factions_summary.json"))?;

    let cards_path = index_dir.join("cards.bin");
    let cards = fs::read(&cards_path).with_context(|| format!("read {}", cards_path.display()))?;
    let expected_len = AppStateInner::expected_cards_len(catalog.total_bit_span);
    if cards.len() as u64 != expected_len {
        bail!(
            "cards.bin size mismatch: expected {expected_len} bytes (total_bit_span {} * {RECORD_SIZE}), got {}",
            catalog.total_bit_span,
            cards.len()
        );
    }
    eprintln!("  cards.bin: {} bytes", cards.len());

    let id_gd_whole = load_id_gd_whole(&index_dir, &idgd_catalog)?;
    eprintln!("  id_gd whole-card bitmaps: {}", id_gd_whole.len());

    let id_gd_per_line = load_id_gd_per_line(&index_dir, &idgd_catalog)?;
    eprintln!("  id_gd per-line bitmaps: {}", id_gd_per_line.len());

    let stats = load_stats(&index_dir, &stats_summary)?;
    eprintln!("  stats field groups: {}", stats.len());

    let factions = load_factions(&index_dir, &factions_summary)?;
    eprintln!("  faction bitmaps: {}", factions.len());

    let set_bitmaps = build_set_bitmaps(&catalog);
    eprintln!(
        "  set bitmaps: {} sets{}",
        set_bitmaps.by_set.len(),
        if set_bitmaps.core_and_coreks.is_some() {
            ", CORE+COREKS combined"
        } else {
            ""
        }
    );

    let name_search_index = build_name_search_index(&catalog);
    eprintln!(
        "  name search index: {} families",
        name_search_index.by_family().len()
    );

    let effects_list = build_effects_list(&idgd_catalog);
    let effects_body = Arc::new(serialize_effects_list(&effects_list)?);
    eprintln!(
        "  effects list: {} triggers, {} conditions, {} outputs ({} bytes JSON)",
        effects_list.triggers.len(),
        effects_list.conditions.len(),
        effects_list.output.len(),
        effects_body.len()
    );

    eprintln!("index load complete");

    Ok(AppState::new(Arc::new(AppStateInner {
        index_dir,
        catalog,
        manifest,
        idgd_catalog,
        stats_summary,
        factions_summary,
        cards,
        id_gd_whole,
        id_gd_per_line,
        stats,
        factions,
        set_bitmaps,
        name_search_index,
        effects_body,
    })))
}

fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn load_roar(path: &Path) -> Result<RoaringBitmap> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    RoaringBitmap::deserialize_from(&bytes[..])
        .with_context(|| format!("deserialize roaring bitmap {}", path.display()))
}

fn load_id_gd_whole(index_dir: &Path, catalog: &IdGdCatalog) -> Result<BTreeMap<u32, RoaringBitmap>> {
    let id_gd_dir = index_dir.join("id_gd");
    let mut map = BTreeMap::new();

    for entry in &catalog.entries {
        if entry.bitmap_bytes == 0 {
            continue;
        }
        let path = id_gd_dir.join(&entry.bitmap_file);
        if !path.is_file() {
            continue;
        }
        let bmp = BitmapStore::load(entry.id_gd, &path)?;
        if !bmp.is_empty() {
            map.insert(entry.id_gd, bmp);
        }
    }

    Ok(map)
}

fn load_id_gd_per_line(index_dir: &Path, catalog: &IdGdCatalog) -> Result<BTreeMap<(u32, EffectLine), RoaringBitmap>> {
    let id_gd_dir = index_dir.join("id_gd");
    let mut map = BTreeMap::new();

    for entry in &catalog.entries {
        for line in EffectLine::ALL {
            let meta = match line {
                EffectLine::M1 => entry.m1.as_ref(),
                EffectLine::M2 => entry.m2.as_ref(),
                EffectLine::M3 => entry.m3.as_ref(),
                EffectLine::Ec => entry.ec.as_ref(),
            };
            let Some(meta) = meta else { continue };
            if meta.bitmap_bytes == 0 {
                continue;
            }
            let path = id_gd_dir.join(&meta.bitmap_file);
            if !path.is_file() {
                continue;
            }
            let bmp = BitmapStore::load(entry.id_gd, &path)?;
            if !bmp.is_empty() {
                map.insert((entry.id_gd, line), bmp);
            }
        }
    }

    Ok(map)
}

fn stat_field_from_dir_name(name: &str) -> Option<StatField> {
    StatField::ALL.into_iter().find(|f| f.dir_name() == name)
}

fn load_stats(index_dir: &Path, summary: &StatsSummary) -> Result<BTreeMap<StatField, [RoaringBitmap; 16]>> {
    let mut out = BTreeMap::new();

    for field_summary in &summary.fields {
        let Some(field) = stat_field_from_dir_name(&field_summary.field) else {
            eprintln!("  warning: unknown stat field {}", field_summary.field);
            continue;
        };

        let mut buckets: [RoaringBitmap; 16] = std::array::from_fn(|_| RoaringBitmap::new());
        let field_dir = index_dir.join(&field_summary.bitmap_dir);

        for (&value, &count) in &field_summary.counts {
            if count == 0 {
                continue;
            }
            let path = field_dir.join(format!("{value:02}.roar"));
            if !path.is_file() {
                eprintln!(
                    "  warning: missing stat bitmap {} (count={count})",
                    path.display()
                );
                continue;
            }
            let bmp = load_roar(&path)?;
            buckets[value as usize] = bmp;
        }

        out.insert(field, buckets);
    }

    Ok(out)
}

fn faction_from_reference(reference: &str) -> Option<Faction> {
    Faction::ALL
        .into_iter()
        .find(|f| f.reference() == reference)
}

fn load_factions(index_dir: &Path, summary: &FactionsSummary) -> Result<BTreeMap<Faction, RoaringBitmap>> {
    let mut out = BTreeMap::new();

    for entry in &summary.factions {
        if entry.card_count == 0 {
            continue;
        }
        let Some(faction) = faction_from_reference(&entry.reference) else {
            eprintln!("  warning: unknown faction {}", entry.reference);
            continue;
        };
        let path = index_dir.join(&entry.bitmap_file);
        if !path.is_file() {
            eprintln!(
                "  warning: missing faction bitmap {} (card_count={})",
                path.display(),
                entry.card_count
            );
            continue;
        }
        let bmp = load_roar(&path)?;
        if !bmp.is_empty() {
            out.insert(faction, bmp);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_for_search_folds_diacritics_and_lowercases() {
        assert_eq!(normalize_name_for_search("Élémentaire"), "elementaire");
        assert_eq!(normalize_name_for_search("elementaire"), "elementaire");
        assert_eq!(
            normalize_name_for_search("Élémentaire de Kélon"),
            "elementaire de kelon"
        );
    }
}
