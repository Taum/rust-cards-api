use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, PutPayload};
use tar::{Builder, Header};
use uniques_http_api::{load_index_from_object_store, ObjectStoreIndexClient};

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

fn pack_fixture_to_bytes(fixture_dir: &Path) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let encoder = zstd::Encoder::new(&mut tar_bytes, 0).expect("create zstd encoder");
        let mut builder = Builder::new(encoder);

        for (relative, bytes) in collect_files(fixture_dir) {
            let archive_name = relative.to_string_lossy().replace('\\', "/");
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
    tar_bytes
}

#[test]
fn loads_minimal_fixture_from_object_store() {
    let archive_bytes = pack_fixture_to_bytes(Path::new(FIXTURE_INDEX));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let client = rt.block_on(async {
        let store = Arc::new(InMemory::new());
        let prefix = ObjectPath::from("index");
        let archive_object = "index/full_index.tar.zst";

        store
            .put(
                &prefix.child("full_index.tar.zst"),
                PutPayload::from_bytes(Bytes::from(archive_bytes)),
            )
            .await
            .expect("put archive");

        let sidecar = serde_json::json!({
            "version": 1_u64,
            "archive_object": archive_object,
        });
        store
            .put(
                &prefix.child("version.json"),
                PutPayload::from_bytes(Bytes::from(serde_json::to_vec(&sidecar).unwrap())),
            )
            .await
            .expect("put version sidecar");

        ObjectStoreIndexClient::from_parts(store, prefix, "memory://index")
    });

    drop(rt);

    let state = load_index_from_object_store(&client).expect("load from object store");

    let index = state.index();
    assert_eq!(index.catalog().set, "TEST");
    assert_eq!(index.catalog().total_bit_span, 1);
    assert!(!index.effects_body().is_empty());
}
