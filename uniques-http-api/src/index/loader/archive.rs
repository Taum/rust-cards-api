use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tar::Archive;

use super::storage::IndexStorage;

pub struct TarZstIndexStorage {
    archive_path: PathBuf,
    entries: HashMap<String, Vec<u8>>,
}

impl TarZstIndexStorage {
    pub fn open(path: &Path) -> Result<Self> {
        let archive_path = path.to_path_buf();
        let file = File::open(&archive_path)
            .with_context(|| format!("open archive {}", archive_path.display()))?;
        let decoder = zstd::Decoder::new(file)
            .with_context(|| format!("open zstd decoder for {}", archive_path.display()))?;
        let mut archive = Archive::new(decoder);

        let mut entries = HashMap::new();
        for entry in archive
            .entries()
            .with_context(|| format!("read tar entries from {}", archive_path.display()))?
        {
            let mut entry = entry.with_context(|| {
                format!("read tar entry from {}", archive_path.display())
            })?;
            if entry.header().entry_type().is_dir() {
                continue;
            }
            let Some(relative_path) = normalize_entry_path(&entry.path()?) else {
                continue;
            };
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .with_context(|| format!("read {relative_path} from {}", archive_path.display()))?;
            entries.insert(relative_path, bytes);
        }

        Ok(Self {
            archive_path,
            entries,
        })
    }
}

impl IndexStorage for TarZstIndexStorage {
    fn source_path(&self) -> &Path {
        &self.archive_path
    }

    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>> {
        self.entries
            .get(relative_path)
            .cloned()
            .with_context(|| {
                format!(
                    "missing {relative_path} in archive {}",
                    self.archive_path.display()
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
