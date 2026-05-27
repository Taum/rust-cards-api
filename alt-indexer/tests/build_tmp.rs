use alt_indexer::build;
use alt_indexer::catalog::Catalog;
use alt_indexer::query;
use std::fs;
use std::path::PathBuf;

fn setup_fixture_dataset() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let cards = [
        ("AX", "06", "ALT_COREKS_B_AX_06_U_5.json"),
        ("MU", "22", "ALT_COREKS_B_MU_22_U_3140.json"),
        ("OR", "16", "ALT_COREKS_B_OR_16_U_6.json"),
    ];

    for (faction, family, file) in cards {
        let dest_dir = root
            .join("json")
            .join("COREKS")
            .join(faction)
            .join(family);
        fs::create_dir_all(&dest_dir).expect("mkdir");
        fs::copy(manifest.join("tmp").join(file), dest_dir.join(file)).expect("copy");
    }

    (dir, root)
}

#[test]
fn build_query_decode_tmp_fixtures() {
    let (_guard, root) = setup_fixture_dataset();
    let out = tempfile::tempdir().expect("out tempdir");
    let summary = build::build(
        &root,
        "COREKS",
        out.path(),
        build::BuildOptions { file_limit: None },
    )
    .expect("build");
    assert_eq!(summary.files_processed, 3);
    assert!(summary.id_gd_count > 0);

    let catalog = Catalog::load(&summary.output_dir.join("catalog.json")).expect("catalog");
    assert_eq!(catalog.families.len(), 3);
    assert_eq!(catalog.total_cards_indexed(), 3);

    let ax = catalog
        .families
        .iter()
        .find(|f| f.family_id == "AX_06")
        .expect("AX_06");
    let decoded = catalog.decode_bit(ax.start_bit + 4).expect("decode AX_06 U_5");
    assert_eq!(decoded.reference, "ALT_COREKS_B_AX_06_U_5");

    let result = query::query_id_gd(out.path(), "COREKS", 90, Some(5)).expect("query");
    assert!(result.cardinality >= 1);

    let idgd_text =
        std::fs::read_to_string(summary.output_dir.join("idgd_catalog.json")).expect("idgd catalog");
    let idgd: serde_json::Value = serde_json::from_str(&idgd_text).expect("parse idgd catalog");
    let entries = idgd["entries"].as_array().expect("entries array");
    assert!(!entries.is_empty());
    let sample = entries
        .iter()
        .find(|e| e["id_gd"].as_u64() == Some(90))
        .expect("idGd 90 entry");
    assert_eq!(sample["card_count"].as_u64(), Some(1));
    assert!(sample["bitmap_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(sample["translations"]["en_US"]["text"].is_string());

    let stats_summary_path = summary.output_dir.join("stats_summary.json");
    assert!(stats_summary_path.is_file(), "stats_summary.json missing");
    let stats_text = fs::read_to_string(&stats_summary_path).expect("stats summary");
    let stats: serde_json::Value = serde_json::from_str(&stats_text).expect("parse stats summary");
    assert_eq!(stats["version"].as_u64(), Some(1));
    assert_eq!(stats["total_cards_indexed"].as_u64(), Some(3));

    let main_cost = stats["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|f| f["field"].as_str() == Some("main_cost"))
        .expect("main_cost field");
    let counts = main_cost["counts"].as_array().expect("counts");
    assert_eq!(counts[2].as_u64(), Some(1)); // AX_06 U_5
    assert_eq!(counts[3].as_u64(), Some(1)); // OR_16 U_6
    assert_eq!(counts[7].as_u64(), Some(1)); // MU_22 U_3140

    assert!(
        summary
            .output_dir
            .join("stats/main_cost/02.roar")
            .is_file()
    );
    assert!(
        summary
            .output_dir
            .join("stats/main_cost/07.roar")
            .is_file()
    );
}
