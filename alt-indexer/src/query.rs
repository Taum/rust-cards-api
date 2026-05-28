use crate::bitmap::BitmapStore;
use crate::catalog::Catalog;
use crate::compact::CompactCardView;
use crate::idgd_catalog::IdGdCatalog;
use anyhow::{Context, Result};
use roaring::RoaringBitmap;
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
    pub cardinality: u64,
    pub recap_lines: Vec<String>,
    pub cards: Vec<EffectCard>,
}

pub fn query_id_gd(
    index_dir: &Path,
    set: &str,
    id_gd: u32,
    list_limit: Option<usize>,
) -> Result<QueryResult> {
    query_id_gds(index_dir, set, &[id_gd], list_limit)
}

pub fn query_id_gd_effect_text(
    index_dir: &Path,
    set: &str,
    id_gd: u32,
    list_limit: Option<usize>,
    locale: &str,
) -> Result<EffectQueryResult> {
    query_id_gds_effect_text(index_dir, set, &[id_gd], list_limit, locale)
}

pub fn query_id_gds(
    index_dir: &Path,
    set: &str,
    id_gds: &[u32],
    list_limit: Option<usize>,
) -> Result<QueryResult> {
    let (bitmap, _recap, _texts) = build_multi_idgd_query(index_dir, set, id_gds, None)?;

    let cardinality = bitmap.len();
    let mut rows = Vec::new();

    let Some(limit) = list_limit else {
        return Ok(QueryResult { cardinality, rows });
    };
    if limit == 0 {
        return Ok(QueryResult { cardinality, rows });
    }

    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_bin_path = set_dir.join("cards.bin");
    let cards_data = std::fs::read(&cards_bin_path)
        .with_context(|| format!("read {}", cards_bin_path.display()))?;

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

    Ok(QueryResult { cardinality, rows })
}

pub fn query_id_gds_effect_text(
    index_dir: &Path,
    set: &str,
    id_gds: &[u32],
    list_limit: Option<usize>,
    locale: &str,
) -> Result<EffectQueryResult> {
    let (bitmap, recap_lines, text_by_id) = build_multi_idgd_query(index_dir, set, id_gds, Some(locale))?;

    let cardinality = bitmap.len();
    let mut cards = Vec::new();

    let Some(limit) = list_limit else {
        return Ok(EffectQueryResult {
            cardinality,
            recap_lines,
            cards,
        });
    };
    if limit == 0 {
        return Ok(EffectQueryResult {
            cardinality,
            recap_lines,
            cards,
        });
    }

    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_bin_path = set_dir.join("cards.bin");
    let cards_data = std::fs::read(&cards_bin_path)
        .with_context(|| format!("read {}", cards_bin_path.display()))?;

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

    Ok(EffectQueryResult {
        cardinality,
        recap_lines,
        cards,
    })
}

fn build_multi_idgd_query(
    index_dir: &Path,
    set: &str,
    id_gds: &[u32],
    locale: Option<&str>,
) -> Result<(RoaringBitmap, Vec<String>, BTreeMap<u32, String>)> {
    anyhow::ensure!(!id_gds.is_empty(), "at least one --id-gd must be provided");

    let set_dir = index_dir.join(set);
    let idgd_catalog_path = set_dir.join("idgd_catalog.json");
    let idgd_catalog_text = std::fs::read_to_string(&idgd_catalog_path)
        .with_context(|| format!("read {}", idgd_catalog_path.display()))?;
    let idgd_catalog: IdGdCatalog = serde_json::from_str(&idgd_catalog_text)
        .with_context(|| format!("parse {}", idgd_catalog_path.display()))?;

    #[derive(Clone)]
    struct MetaEntry {
        element_type: String,
        translations: BTreeMap<String, crate::card::LocaleText>,
    }

    let mut meta_by_id: BTreeMap<u32, MetaEntry> = BTreeMap::new();
    for e in idgd_catalog.entries {
        meta_by_id.insert(
            e.id_gd,
            MetaEntry {
                element_type: e.element_type,
                translations: e.translations,
            },
        );
    }

    // Preserve input order (dedupe per bucket).
    let mut triggers: Vec<u32> = Vec::new();
    let mut conditions: Vec<u32> = Vec::new();
    let mut outputs: Vec<u32> = Vec::new();

    for &id in id_gds {
        let meta = meta_by_id
            .get(&id)
            .with_context(|| format!("idGd {id} not found in {}", idgd_catalog_path.display()))?;
        match meta.element_type.as_str() {
            "TRIGGER" => {
                if !triggers.contains(&id) {
                    triggers.push(id);
                }
            }
            "CONDITION" => {
                if !conditions.contains(&id) {
                    conditions.push(id);
                }
            }
            "OUTPUT" => {
                if !outputs.contains(&id) {
                    outputs.push(id);
                }
            }
            other => {
                anyhow::bail!("idGd {id} has unsupported element_type={other}");
            }
        }
    }

    anyhow::ensure!(
        !triggers.is_empty() || !conditions.is_empty() || !outputs.is_empty(),
        "no valid idGd values were provided"
    );

    // Build text map (used both for recap and effect decoding).
    let mut text_by_id: BTreeMap<u32, String> = BTreeMap::new();
    if let Some(locale) = locale {
        for (&id, meta) in &meta_by_id {
            let t = pick_translation(&meta.translations, locale);
            if !t.is_empty() {
                text_by_id.insert(id, t);
            }
        }
    }

    let id_gd_dir = set_dir.join("id_gd");

    let mut groups: Vec<RoaringBitmap> = Vec::new();
    if !triggers.is_empty() {
        groups.push(union_bitmaps(&id_gd_dir, &triggers)?);
    }
    if !conditions.is_empty() {
        groups.push(union_bitmaps(&id_gd_dir, &conditions)?);
    }
    if !outputs.is_empty() {
        groups.push(union_bitmaps(&id_gd_dir, &outputs)?);
    }

    let mut it = groups.into_iter();
    let mut bitmap = it.next().unwrap_or_else(RoaringBitmap::new);
    for g in it {
        bitmap &= g;
    }

    let mut recap_lines: Vec<String> = Vec::new();
    if locale.is_some() {
        recap_lines.push("Searching for cards matching:".to_string());
        if !triggers.is_empty() {
            recap_lines.push("Triggers is one of :".to_string());
            for id in &triggers {
                let t = text_by_id.get(id).cloned().unwrap_or_default();
                recap_lines.push(format!("- \"{t}\""));
            }
        }
        if !conditions.is_empty() {
            recap_lines.push("Condition is one of :".to_string());
            for id in &conditions {
                let t = text_by_id.get(id).cloned().unwrap_or_default();
                recap_lines.push(format!("- \"{t}\""));
            }
        }
        if !outputs.is_empty() {
            recap_lines.push("Output is one of :".to_string());
            for id in &outputs {
                let t = text_by_id.get(id).cloned().unwrap_or_default();
                recap_lines.push(format!("- \"{t}\""));
            }
        }
    }

    Ok((bitmap, recap_lines, text_by_id))
}

fn union_bitmaps(id_gd_dir: &Path, ids: &[u32]) -> Result<RoaringBitmap> {
    let mut merged = RoaringBitmap::new();
    for &id in ids {
        let bitmap_path = id_gd_dir.join(format!("{id}.roar"));
        let bitmap = BitmapStore::load(id, &bitmap_path)
            .with_context(|| format!("idGd {id} not found at {}", bitmap_path.display()))?;
        merged |= bitmap;
    }
    Ok(merged)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::LocaleText;
    use crate::idgd_catalog::{IdGdCatalog, IdGdCatalogEntry};
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    fn write_bitmap(path: &Path, values: &[u32]) -> Result<()> {
        let mut bmp = RoaringBitmap::new();
        for &v in values {
            bmp.insert(v);
        }
        let mut bytes = Vec::new();
        bmp.serialize_into(&mut bytes)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    fn lt(locale: &str, text: &str) -> LocaleText {
        LocaleText {
            locale: locale.to_string(),
            text: text.to_string(),
        }
    }

    fn write_idgd_catalog(set_dir: &Path, entries: Vec<(u32, &str, &str)>) -> Result<()> {
        // entries: (id, element_type, en_US_text)
        let mut cat_entries = Vec::new();
        for (id, element_type, text) in entries {
            let mut translations: BTreeMap<String, LocaleText> = BTreeMap::new();
            translations.insert("en_US".to_string(), lt("en_US", text));
            cat_entries.push(IdGdCatalogEntry {
                id_gd: id,
                card_count: 0,
                bitmap_bytes: 0,
                bitmap_file: format!("{id}.roar"),
                element_type: element_type.to_string(),
                translations,
            });
        }
        let cat = IdGdCatalog {
            set: "TEST".to_string(),
            entries: cat_entries,
        };
        let text = serde_json::to_string_pretty(&cat)?;
        fs::write(set_dir.join("idgd_catalog.json"), text)?;
        Ok(())
    }

    fn setup_index() -> Result<(TempDir, std::path::PathBuf)> {
        let td = TempDir::new()?;
        let set_dir = td.path().join("SET");
        fs::create_dir_all(set_dir.join("id_gd"))?;
        Ok((td, set_dir))
    }

    #[test]
    fn unions_within_bucket() -> Result<()> {
        let (_td, set_dir) = setup_index()?;

        // Two triggers whose bitmaps overlap.
        write_idgd_catalog(
            &set_dir,
            vec![(1, "TRIGGER", "{J}"), (2, "TRIGGER", "When I go...")],
        )?;
        write_bitmap(&set_dir.join("id_gd/1.roar"), &[1, 2])?;
        write_bitmap(&set_dir.join("id_gd/2.roar"), &[2, 3])?;

        let (bmp, _recap, _texts) = build_multi_idgd_query(set_dir.parent().unwrap(), "SET", &[1, 2], None)?;
        assert_eq!(bmp.len(), 3);
        assert!(bmp.contains(1));
        assert!(bmp.contains(2));
        assert!(bmp.contains(3));
        Ok(())
    }

    #[test]
    fn intersects_across_buckets() -> Result<()> {
        let (_td, set_dir) = setup_index()?;

        write_idgd_catalog(
            &set_dir,
            vec![
                (1, "TRIGGER", "T1"),
                (2, "TRIGGER", "T2"),
                (3, "CONDITION", "C3"),
            ],
        )?;
        write_bitmap(&set_dir.join("id_gd/1.roar"), &[1, 2])?;
        write_bitmap(&set_dir.join("id_gd/2.roar"), &[2, 3])?;
        write_bitmap(&set_dir.join("id_gd/3.roar"), &[2, 3])?;

        let (bmp, _recap, _texts) =
            build_multi_idgd_query(set_dir.parent().unwrap(), "SET", &[1, 2, 3], None)?;
        // triggers union = {1,2,3}; conditions union = {2,3}; intersect = {2,3}
        assert_eq!(bmp.len(), 2);
        assert!(bmp.contains(2));
        assert!(bmp.contains(3));
        assert!(!bmp.contains(1));
        Ok(())
    }

    #[test]
    fn ignores_missing_bucket() -> Result<()> {
        let (_td, set_dir) = setup_index()?;

        write_idgd_catalog(&set_dir, vec![(10, "OUTPUT", "Draw a card")])?;
        write_bitmap(&set_dir.join("id_gd/10.roar"), &[7, 8, 9])?;

        let (bmp, _recap, _texts) =
            build_multi_idgd_query(set_dir.parent().unwrap(), "SET", &[10], None)?;
        assert_eq!(bmp.len(), 3);
        Ok(())
    }

    #[test]
    fn errors_on_unknown_element_type() -> Result<()> {
        let (_td, set_dir) = setup_index()?;

        write_idgd_catalog(&set_dir, vec![(99, "UNKNOWN", "???")])?;
        write_bitmap(&set_dir.join("id_gd/99.roar"), &[1])?;

        let err = build_multi_idgd_query(set_dir.parent().unwrap(), "SET", &[99], None)
            .expect_err("should fail");
        assert!(format!("{err:#}").contains("unsupported element_type"));
        Ok(())
    }

    #[test]
    fn recap_groups_by_bucket_in_input_order() -> Result<()> {
        let (_td, set_dir) = setup_index()?;

        write_idgd_catalog(
            &set_dir,
            vec![
                (1, "TRIGGER", "{J}"),
                (2, "TRIGGER", "When I go to reserve from the Expedition"),
                (3, "CONDITION", "You may pay {1} if you do"),
                (4, "OUTPUT", "Draw a card"),
                (5, "OUTPUT", "You may discard target Permanent"),
            ],
        )?;
        // Bitmaps not relevant to recap formatting, but required on disk.
        write_bitmap(&set_dir.join("id_gd/1.roar"), &[1])?;
        write_bitmap(&set_dir.join("id_gd/2.roar"), &[1])?;
        write_bitmap(&set_dir.join("id_gd/3.roar"), &[1])?;
        write_bitmap(&set_dir.join("id_gd/4.roar"), &[1])?;
        write_bitmap(&set_dir.join("id_gd/5.roar"), &[1])?;

        let (_bmp, recap, _texts) = build_multi_idgd_query(
            set_dir.parent().unwrap(),
            "SET",
            &[1, 2, 3, 4, 5],
            Some("en_US"),
        )?;

        let joined = recap.join("\n");
        assert!(joined.contains("Searching for cards matching:"));
        assert!(joined.contains("Triggers is one of :"));
        assert!(joined.contains("- \"{J}\""));
        assert!(joined.contains("- \"When I go to reserve from the Expedition\""));
        assert!(joined.contains("Condition is one of :"));
        assert!(joined.contains("- \"You may pay {1} if you do\""));
        assert!(joined.contains("Output is one of :"));
        assert!(joined.contains("- \"Draw a card\""));
        assert!(joined.contains("- \"You may discard target Permanent\""));
        Ok(())
    }
}
