use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::storage::IndexStorage;

pub struct DiskIndexStorage {
    root: PathBuf,
}

impl DiskIndexStorage {
    pub fn new(path: &Path) -> Result<Self> {
        let root = path
            .canonicalize()
            .with_context(|| format!("resolve index path {}", path.display()))?;
        Ok(Self { root })
    }

    fn resolve(&self, relative_path: &str) -> PathBuf {
        self.root.join(relative_path)
    }
}

impl IndexStorage for DiskIndexStorage {
    fn source_path(&self) -> &Path {
        &self.root
    }

    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>> {
        let path = self.resolve(relative_path);
        fs::read(&path).with_context(|| format!("read {}", path.display()))
    }

    fn has_file(&self, relative_path: &str) -> bool {
        self.resolve(relative_path).is_file()
    }
}
