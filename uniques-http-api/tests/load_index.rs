use std::path::Path;

use uniques_http_api::load_index;

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");

#[test]
fn loads_minimal_fixture_index() {
    let state = load_index(Path::new(FIXTURE_INDEX)).expect("load index");
    let index = state.index();

    assert_eq!(index.catalog().set, "TEST");
    assert_eq!(index.catalog().total_bit_span, 1);
    assert_eq!(index.cards().len(), 32);
    assert!(index.id_gd_whole().is_empty());
    assert!(index.id_gd_per_line().is_empty());
    assert!(index.stats().is_empty());
    assert!(index.factions().is_empty());
    assert_eq!(index.name_search_index().by_family().len(), 1);
    assert_eq!(index.family_lookup_index().len(), 1);
    assert_eq!(
        index.family_lookup_index().len(),
        index.catalog().families.len()
    );
    assert!(!index.effects_body().is_empty());

    let reference = index.decode_reference(0).expect("decode card 0");
    assert_eq!(reference, "ALT_TEST_B_AX_04_U_1");
}
