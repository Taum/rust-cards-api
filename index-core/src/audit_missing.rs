use crate::catalog::{Catalog, FamilyEntry};
use crate::compact::{CompactCardView, RECORD_SIZE};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Default)]
struct FamilyAudit {
    key: String,
    missing: Vec<String>,
    corrupt: Vec<(u32, String)>,
}

pub fn run(index_dir: &Path, set: &str, json: bool) -> Result<()> {
    let set_dir = index_dir.join(set);
    let catalog_path = set_dir.join("catalog.json");
    let cards_bin_path = set_dir.join("cards.bin");

    let catalog = Catalog::load(&catalog_path)
        .with_context(|| format!("load catalog {}", catalog_path.display()))?;
    let cards_data = std::fs::read(&cards_bin_path)
        .with_context(|| format!("read {}", cards_bin_path.display()))?;

    let expected_len = catalog.total_bit_span as usize * RECORD_SIZE;
    if cards_data.len() < expected_len {
        bail!(
            "cards.bin size {} is smaller than expected {} (total_bit_span {} × record size {})",
            cards_data.len(),
            expected_len,
            catalog.total_bit_span,
            RECORD_SIZE
        );
    }

    let mut by_key: BTreeMap<String, FamilyAudit> = BTreeMap::new();

    for family in catalog
        .families
        .iter()
        .filter(|f| f.max_unique_id != f.card_count)
    {
        let key = family_output_key(&catalog, family);
        let entry = by_key.entry(key.clone()).or_insert_with(|| FamilyAudit {
            key,
            ..Default::default()
        });

        let start = family.start_bit;
        let end = family.start_bit + family.max_unique_id;
        for card_index in start..end {
            let Some(view) = CompactCardView::from_data(&cards_data, card_index) else {
                continue;
            };
            if view.faction_code() != 0 {
                continue;
            }

            let decoded = catalog.decode_bit(card_index)?;
            if record_tail_nonzero(view.as_bytes()) {
                entry.corrupt.push((card_index, decoded.reference));
            } else {
                entry.missing.push(decoded.reference);
            }
        }
    }

    if json {
        print_json(&by_key)?;
    } else {
        print_text(&by_key)?;
    }

    for audit in by_key.values() {
        for (card_index, reference) in &audit.corrupt {
            eprintln!(
                "ERROR: nonzero bytes in zero-faction record: index={card_index} ref={reference}"
            );
        }
    }

    Ok(())
}

pub fn family_output_key(catalog: &Catalog, family: &FamilyEntry) -> String {
    let set = family.source_set.as_deref().unwrap_or(&catalog.set);
    format!("ALT_{set}_B_{}", family.family_id)
}

fn record_tail_nonzero(record: &[u8; RECORD_SIZE]) -> bool {
    record[1..].iter().any(|b| *b != 0)
}

fn print_json(by_key: &BTreeMap<String, FamilyAudit>) -> Result<()> {
    let mut out: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (key, audit) in by_key {
        if audit.missing.is_empty() {
            continue;
        }
        out.insert(
            key.clone(),
            audit.missing.iter().map(String::as_str).collect(),
        );
    }
    let text = serde_json::to_string_pretty(&out)?;
    println!("{text}");
    Ok(())
}

fn print_text(by_key: &BTreeMap<String, FamilyAudit>) -> Result<()> {
    if by_key.is_empty() {
        println!("no families with max_unique_id != card_count");
        return Ok(());
    }

    let mut total_missing = 0usize;
    for audit in by_key.values() {
        if audit.missing.is_empty() {
            continue;
        }
        println!(
            "{} ({} missing)",
            audit.key,
            audit.missing.len()
        );
        for reference in &audit.missing {
            println!("  {reference}");
        }
        total_missing += audit.missing.len();
    }

    if total_missing == 0 {
        println!("no missing records found in gap-suspect families");
    } else {
        println!();
        println!("total missing: {total_missing}");
    }

    let _ = io::stdout().flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::compact::{encode_record, CompactCardFields};
    use std::fs;
    use tempfile::TempDir;

    fn sample_fields(faction: u8) -> CompactCardFields {
        CompactCardFields {
            faction_code: faction,
            main_cost: 1,
            recall_cost: 2,
            mountain_power: 3,
            ocean_power: 4,
            forest_power: 5,
            main_effect: [[0; 3]; 3],
            echo_effect: [0; 3],
        }
    }

    fn write_mini_index(td: &TempDir) {
        let set_dir = td.path().join("TEST");
        fs::create_dir_all(&set_dir).unwrap();

        let catalog_json = r#"{
  "set": "TEST",
  "faction_order": ["AX"],
  "families": [{
    "start_bit": 0,
    "faction": "AX",
    "family_number": "01",
    "family_id": "AX_01",
    "max_unique_id": 4,
    "card_count": 2,
    "first_reference": "ALT_TEST_B_AX_01_U_1",
    "name": { "en_US": "Test" },
    "artist": "",
    "card_sub_types": [],
    "set": { "reference": "TEST", "name": "Test", "code": null }
  }],
  "total_bit_span": 4
}"#;
        fs::write(set_dir.join("catalog.json"), catalog_json).unwrap();

        let mut data = vec![0u8; 4 * RECORD_SIZE];
        let present = encode_record(&sample_fields(1));
        data[0..RECORD_SIZE].copy_from_slice(&present);
        data[2 * RECORD_SIZE..3 * RECORD_SIZE].copy_from_slice(&present);
        let mut corrupt = [0u8; RECORD_SIZE];
        corrupt[1] = 7;
        data[3 * RECORD_SIZE..4 * RECORD_SIZE].copy_from_slice(&corrupt);
        fs::write(set_dir.join("cards.bin"), data).unwrap();
    }

    #[test]
    fn detects_missing_and_corrupt_records() -> Result<()> {
        let td = TempDir::new()?;
        write_mini_index(&td);
        let set_dir = td.path().join("TEST");
        let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
        let cards_data = fs::read(set_dir.join("cards.bin"))?;
        let family = &catalog.families[0];

        let missing_view = CompactCardView::from_data(&cards_data, family.start_bit + 1).unwrap();
        assert_eq!(missing_view.faction_code(), 0);
        assert!(!record_tail_nonzero(missing_view.as_bytes()));

        let corrupt_view = CompactCardView::from_data(&cards_data, family.start_bit + 3).unwrap();
        assert_eq!(corrupt_view.faction_code(), 0);
        assert!(record_tail_nonzero(corrupt_view.as_bytes()));

        Ok(())
    }

    #[test]
    fn family_output_key_matches_reference_prefix() {
        let td = TempDir::new().unwrap();
        write_mini_index(&td);
        let catalog =
            Catalog::load(&td.path().join("TEST/catalog.json")).unwrap();
        let key = family_output_key(&catalog, &catalog.families[0]);
        assert_eq!(key, "ALT_TEST_B_AX_01");
    }
}
