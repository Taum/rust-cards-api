use std::fs;
use std::process::Command;

use alt_indexer::audit_missing;
use alt_indexer::catalog::Catalog;
use alt_indexer::compact::{encode_record, CompactCardFields, RECORD_SIZE};
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
fn audit_missing_lists_gap_records() {
    let td = TempDir::new().expect("tempdir");
    write_mini_index(&td);

    audit_missing::run(td.path(), "TEST", false).expect("audit-missing");

    let catalog = Catalog::load(&td.path().join("TEST/catalog.json")).expect("catalog");
    let key = audit_missing::family_output_key(&catalog, &catalog.families[0]);
    assert_eq!(key, "ALT_TEST_B_AX_01");
}

#[test]
fn audit_missing_cli_json_output() {
    let td = TempDir::new().expect("tempdir");
    write_mini_index(&td);

    let output = Command::new(env!("CARGO_BIN_EXE_alt-indexer"))
        .args([
            "audit-missing",
            "--index-dir",
            td.path().to_str().unwrap(),
            "--set",
            "TEST",
            "--json",
        ])
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ALT_TEST_B_AX_01\""));
    assert!(stdout.contains("ALT_TEST_B_AX_01_U_2"));
    assert!(!stdout.contains("ALT_TEST_B_AX_01_U_4"));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ERROR"));
    assert!(stderr.contains("ALT_TEST_B_AX_01_U_4"));
}
