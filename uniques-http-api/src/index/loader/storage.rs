use std::path::Path;

use anyhow::{Context, Result};
use roaring::RoaringBitmap;
use serde::Deserialize;

pub trait IndexStorage {
    /// Canonical directory or archive path for logs and `UniquesIndex.index_dir`.
    fn source_path(&self) -> &Path;

    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>>;
    fn has_file(&self, relative_path: &str) -> bool;
}

pub fn read_text(storage: &impl IndexStorage, relative_path: &str) -> Result<String> {
    let bytes = storage
        .read_bytes(relative_path)
        .with_context(|| format!("read {relative_path} from {}", storage.source_path().display()))?;
    String::from_utf8(bytes).with_context(|| {
        format!(
            "decode {relative_path} as UTF-8 from {}",
            storage.source_path().display()
        )
    })
}

pub fn read_json<T: for<'de> Deserialize<'de>>(
    storage: &impl IndexStorage,
    relative_path: &str,
) -> Result<T> {
    let text = read_text(storage, relative_path)?;
    serde_json::from_str(&text).with_context(|| {
        format!(
            "parse {relative_path} from {}",
            storage.source_path().display()
        )
    })
}

pub fn read_roar(storage: &impl IndexStorage, relative_path: &str) -> Result<RoaringBitmap> {
    let bytes = storage
        .read_bytes(relative_path)
        .with_context(|| format!("read {relative_path} from {}", storage.source_path().display()))?;
    RoaringBitmap::deserialize_from(&bytes[..]).with_context(|| {
        format!(
            "deserialize roaring bitmap {relative_path} from {}",
            storage.source_path().display()
        )
    })
}

pub fn read_roar_id_gd(
    storage: &impl IndexStorage,
    id_gd: u32,
    relative_path: &str,
) -> Result<RoaringBitmap> {
    let bytes = storage
        .read_bytes(relative_path)
        .with_context(|| format!("read {relative_path} from {}", storage.source_path().display()))?;
    RoaringBitmap::deserialize_from(&bytes[..]).with_context(|| {
        format!(
            "deserialize idGd {id_gd} from {relative_path} ({})",
            storage.source_path().display()
        )
    })
}
