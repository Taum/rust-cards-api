use crate::bitmap::BitmapStore;
use crate::catalog::Catalog;
use crate::compact::CompactCardView;
use crate::idgd_catalog::IdGdCatalog;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

pub struct QueryRow {
    pub card_index: u32,
    pub reference: String,
    pub hand: u8,
    pub reserve: u8,
    pub m: u8,
    pub o: u8,
    pub f: u8,
    pub main_effect: String,
    pub echo_effect: String,
}

pub struct QueryResult {
    pub id_gd: u32,
    pub cardinality: u64,
    pub rows: Vec<QueryRow>,
}

pub struct EffectCard {
    pub reference: String,
    pub hand: u8,
    pub reserve: u8,
    pub m: u8,
    pub o: u8,
    pub f: u8,
    pub effect_lines: Vec<String>,
}

pub struct EffectQueryResult {
    pub id_gd: u32,
    pub cardinality: u64,
    pub cards: Vec<EffectCard>,
}

pub fn query_id_gd(
    index_dir: &Path,
    set: &str,
    id_gd: u32,
    list_limit: Option<usize>,
) -> Result<QueryResult> {
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_bin_path = set_dir.join("cards.bin");
    let cards_data = std::fs::read(&cards_bin_path)
        .with_context(|| format!("read {}", cards_bin_path.display()))?;

    let bitmap_path = set_dir.join("id_gd").join(format!("{id_gd}.roar"));
    let bitmap = BitmapStore::load(id_gd, &bitmap_path)
        .with_context(|| format!("idGd {id_gd} not found at {}", bitmap_path.display()))?;

    let cardinality = bitmap.len();
    let mut rows = Vec::new();

    if let Some(limit) = list_limit {
        for bit in bitmap.iter().take(limit) {
            let decoded = catalog.decode_bit(bit)?;
            let view = match CompactCardView::from_data(&cards_data, bit) {
                Some(v) => v,
                None => continue,
            };

            // Stats mapping: main_cost -> hand, recall_cost -> reserve,
            // mountain/ocean/forest -> M/O/F.
            let hand = view.main_cost();
            let reserve = view.recall_cost();
            let m = view.mountain_power();
            let o = view.ocean_power();
            let f = view.forest_power();

            // MAIN_EFFECT as "t_c_o, t_c_o, t_c_o" (skip all-zero groups).
            let mut main_parts = Vec::new();
            for g in 0..3 {
                let [t, c, e] = view.main_effect_group(g);
                if t == 0 && c == 0 && e == 0 {
                    continue;
                }
                main_parts.push(format!("{t}_{c}_{e}"));
            }
            let main_effect = main_parts.join(", ");

            // ECHO_EFFECT as "t_c_o" if non-zero, otherwise empty.
            let [et, ec, ee] = view.echo_effect();
            let echo_effect = if et == 0 && ec == 0 && ee == 0 {
                String::new()
            } else {
                format!("{et}_{ec}_{ee}")
            };

            rows.push(QueryRow {
                card_index: bit,
                reference: decoded.reference,
                hand,
                reserve,
                m,
                o,
                f,
                main_effect,
                echo_effect,
            });
        }
    }

    Ok(QueryResult {
        id_gd,
        cardinality,
        rows,
    })
}

pub fn query_id_gd_effect_text(
    index_dir: &Path,
    set: &str,
    id_gd: u32,
    list_limit: Option<usize>,
    locale: &str,
) -> Result<EffectQueryResult> {
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_bin_path = set_dir.join("cards.bin");
    let cards_data = std::fs::read(&cards_bin_path)
        .with_context(|| format!("read {}", cards_bin_path.display()))?;

    let idgd_catalog_path = set_dir.join("idgd_catalog.json");
    let idgd_catalog_text = std::fs::read_to_string(&idgd_catalog_path)
        .with_context(|| format!("read {}", idgd_catalog_path.display()))?;
    let idgd_catalog: IdGdCatalog = serde_json::from_str(&idgd_catalog_text)
        .with_context(|| format!("parse {}", idgd_catalog_path.display()))?;

    let text_by_id: BTreeMap<u32, String> = idgd_catalog
        .entries
        .into_iter()
        .map(|e| {
            let t = pick_translation(&e.translations, locale);
            (e.id_gd, t)
        })
        .collect();

    let bitmap_path = set_dir.join("id_gd").join(format!("{id_gd}.roar"));
    let bitmap = BitmapStore::load(id_gd, &bitmap_path)
        .with_context(|| format!("idGd {id_gd} not found at {}", bitmap_path.display()))?;

    let cardinality = bitmap.len();
    let mut cards = Vec::new();

    if let Some(limit) = list_limit {
        for bit in bitmap.iter().take(limit) {
            let decoded = catalog.decode_bit(bit)?;
            let view = match CompactCardView::from_data(&cards_data, bit) {
                Some(v) => v,
                None => continue,
            };

            let hand = view.main_cost();
            let reserve = view.recall_cost();
            let m = view.mountain_power();
            let o = view.ocean_power();
            let f = view.forest_power();

            let mut effect_lines = Vec::new();

            // 3 MAIN_EFFECT lines, skipping empty groups
            for g in 0..3 {
                let [t, c, e] = view.main_effect_group(g);
                let line = build_effect_line(t, c, e, &text_by_id);
                if let Some(line) = line {
                    effect_lines.push(line);
                }
            }

            // ECHO line, skip if empty
            let [t, c, e] = view.echo_effect();
            if let Some(line) = build_effect_line(t, c, e, &text_by_id) {
                effect_lines.push(line);
            }

            cards.push(EffectCard {
                reference: decoded.reference,
                hand,
                reserve,
                m,
                o,
                f,
                effect_lines,
            });
        }
    }

    Ok(EffectQueryResult {
        id_gd,
        cardinality,
        cards,
    })
}

fn pick_translation(map: &BTreeMap<String, crate::card::LocaleText>, locale: &str) -> String {
    if let Some(t) = map.get(locale) {
        return t.text.clone();
    }
    if let Some(t) = map.get("en_US") {
        return t.text.clone();
    }
    map.values().next().map(|t| t.text.clone()).unwrap_or_default()
}

fn build_effect_line(t: u16, c: u16, e: u16, text_by_id: &BTreeMap<u32, String>) -> Option<String> {
    if t == 0 && c == 0 && e == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if t != 0 {
        parts.push(text_by_id.get(&(t as u32)).cloned().unwrap_or_default());
    }
    if c != 0 {
        parts.push(text_by_id.get(&(c as u32)).cloned().unwrap_or_default());
    }
    if e != 0 {
        parts.push(text_by_id.get(&(e as u32)).cloned().unwrap_or_default());
    }
    let line = parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if line.is_empty() { None } else { Some(line) }
}
