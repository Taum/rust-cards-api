use std::path::Path;

use uniques_http_api::load_index;

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");

#[test]
fn loads_minimal_fixture_index() {
    let state = load_index(Path::new(FIXTURE_INDEX)).expect("load index");

    assert_eq!(state.catalog().set, "TEST");
    assert_eq!(state.catalog().total_bit_span, 1);
    assert_eq!(state.cards().len(), 32);
    assert!(state.id_gd_whole().is_empty());
    assert!(state.id_gd_per_line().is_empty());
    assert!(state.stats().is_empty());
    assert!(state.factions().is_empty());
    assert_eq!(state.name_search_index().by_family().len(), 1);
    assert!(!state.effects_body().is_empty());

    let reference = state.decode_reference(0).expect("decode card 0");
    assert_eq!(reference, "ALT_TEST_B_AX_04_U_1");
}
