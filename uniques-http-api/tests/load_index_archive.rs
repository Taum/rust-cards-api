use std::fs::{self, File};
use std::path::{Path, PathBuf};

use tar::{Builder, Header};
use uniques_http_api::load_index;

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");

fn collect_files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    collect_files_rec(dir, dir, &mut out);
    out
}

fn collect_files_rec(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("read fixture dir") {
        let entry = entry.expect("read fixture entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_rec(root, &path, out);
        } else {
            let relative = path.strip_prefix(root).expect("strip fixture prefix");
            let bytes = fs::read(&path).expect("read fixture file");
            out.push((relative.to_path_buf(), bytes));
        }
    }
}

fn pack_fixture_to_tar_zst(
    fixture_dir: &Path,
    archive_path: &Path,
    prefix_entries_with_dot: bool,
) {
    let file = File::create(archive_path).expect("create archive");
    let encoder = zstd::Encoder::new(file, 0).expect("create zstd encoder");
    let mut builder = Builder::new(encoder);

    for (relative, bytes) in collect_files(fixture_dir) {
        let archive_name = if prefix_entries_with_dot {
            format!("./{}", relative.to_string_lossy().replace('\\', "/"))
        } else {
            relative.to_string_lossy().replace('\\', "/")
        };

        let mut header = Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, archive_name, &bytes[..])
            .expect("append tar entry");
    }

    let encoder = builder.into_inner().expect("finish tar");
    encoder.finish().expect("finish zstd");
}

fn assert_loaded_minimal_index(state: &uniques_http_api::AppState) {
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

#[test]
fn loads_minimal_fixture_from_tar_zst_with_clean_paths() {
    let temp_dir = std::env::temp_dir().join(format!(
        "uniques_http_api_archive_test_{}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let archive_path = temp_dir.join("minimal_index.tar.zst");
    pack_fixture_to_tar_zst(Path::new(FIXTURE_INDEX), &archive_path, false);

    let state = load_index(&archive_path).expect("load index from clean archive");
    assert_loaded_minimal_index(&state);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn loads_minimal_fixture_from_tar_zst_with_dot_prefix_paths() {
    let temp_dir = std::env::temp_dir().join(format!(
        "uniques_http_api_archive_legacy_test_{}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let archive_path = temp_dir.join("minimal_index_legacy.tar.zst");
    pack_fixture_to_tar_zst(Path::new(FIXTURE_INDEX), &archive_path, true);

    let state = load_index(&archive_path).expect("load index from legacy archive");
    assert_loaded_minimal_index(&state);

    let _ = fs::remove_dir_all(&temp_dir);
}
