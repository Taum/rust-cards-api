use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tar::Archive;

use super::storage::IndexStorage;

pub struct TarZstIndexStorage {
    source_label: PathBuf,
    entries: HashMap<String, Vec<u8>>,
}

impl TarZstIndexStorage {
    pub fn open(path: &Path) -> Result<Self> {
        let source_label = path.to_path_buf();
        let file = File::open(&source_label)
            .with_context(|| format!("open archive {}", source_label.display()))?;
        Self::open_from_reader(file, source_label)
    }

    pub fn from_bytes(bytes: &[u8], source_label: impl Into<PathBuf>) -> Result<Self> {
        Self::open_from_reader(Cursor::new(bytes), source_label.into())
    }

    fn open_from_reader(reader: impl Read, source_label: PathBuf) -> Result<Self> {
        let decoder = zstd::Decoder::new(reader)
            .with_context(|| format!("open zstd decoder for {}", source_label.display()))?;
        let mut archive = Archive::new(decoder);

        let mut entries = HashMap::new();
        for entry in archive
            .entries()
            .with_context(|| format!("read tar entries from {}", source_label.display()))?
        {
            let mut entry = entry.with_context(|| {
                format!("read tar entry from {}", source_label.display())
            })?;
            if entry.header().entry_type().is_dir() {
                continue;
            }
            let Some(relative_path) = normalize_entry_path(&entry.path()?) else {
                continue;
            };
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).with_context(|| {
                format!(
                    "read {relative_path} from {}",
                    source_label.display()
                )
            })?;
            entries.insert(relative_path, bytes);
        }

        Ok(Self {
            source_label,
            entries,
        })
    }
}

impl IndexStorage for TarZstIndexStorage {
    fn source_path(&self) -> &Path {
        &self.source_label
    }

    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>> {
        self.entries
            .get(relative_path)
            .cloned()
            .with_context(|| {
                format!(
                    "missing {relative_path} in archive {}",
                    self.source_label.display()
                )
            })
    }

    fn has_file(&self, relative_path: &str) -> bool {
        self.entries.contains_key(relative_path)
    }
}

/// Strip a leading `./`, normalize to forward slashes, skip directory-only entries.
pub(crate) fn normalize_entry_path(path: &Path) -> Option<String> {
    let mut components = path.components().peekable();
    if matches!(components.peek(), Some(std::path::Component::CurDir)) {
        components.next();
    }

    let normalized = components
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    if normalized.is_empty() {
        return None;
    }

    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_entry_path_strips_dot_prefix_and_slashes() {
        assert_eq!(
            normalize_entry_path(Path::new("./catalog.json")).as_deref(),
            Some("catalog.json")
        );
        assert_eq!(
            normalize_entry_path(Path::new("./id_gd/1.roar")).as_deref(),
            Some("id_gd/1.roar")
        );
        assert_eq!(normalize_entry_path(Path::new(".")), None);
    }
}
