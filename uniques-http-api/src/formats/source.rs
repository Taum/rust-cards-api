use std::path::{Path, PathBuf};

use crate::config::FormatsSourceConfig;

#[derive(Debug, Clone)]
pub struct DiskFormatsSource {
    root: PathBuf,
}

impl DiskFormatsSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { root: path.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Clone)]
pub enum FormatsSource {
    Disk(DiskFormatsSource),
}

impl FormatsSource {
    pub fn from_config(cfg: &FormatsSourceConfig) -> Self {
        match cfg {
            FormatsSourceConfig::Disk { path } => Self::Disk(DiskFormatsSource::new(path.clone())),
        }
    }
}
