use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const EXTRA_CATALOG_VERSION: u32 = 1;
pub const EXTRA_DIR: &str = "extra";
pub const EXTRA_CATALOG_FILE: &str = "extra_catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtraFilterType {
    Format,
    Property,
}

impl std::str::FromStr for ExtraFilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "format" => Ok(Self::Format),
            "property" => Ok(Self::Property),
            _ => Err(format!("unknown filter type: {s} (expected format or property)")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraCatalogEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<ExtraFilterType>,
    pub negated: bool,
    pub card_count: u64,
    pub bitmap_bytes: u64,
    pub bitmap_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraCatalog {
    pub version: u32,
    pub set: String,
    pub entries: Vec<ExtraCatalogEntry>,
}

impl ExtraCatalog {
    pub fn new(set: impl Into<String>) -> Self {
        Self {
            version: EXTRA_CATALOG_VERSION,
            set: set.into(),
            entries: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
    }

    pub fn contains_id(&self, filter_id: &str) -> bool {
        self.entries.iter().any(|e| e.id == filter_id)
    }

    pub fn append_entry(&mut self, entry: ExtraCatalogEntry) -> Result<()> {
        if self.contains_id(&entry.id) {
            bail!("extra filter id already exists: {}", entry.id);
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Insert or replace an entry with the same `id`.
    pub fn upsert_entry(&mut self, entry: ExtraCatalogEntry) -> bool {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
            true
        } else {
            self.entries.push(entry);
            false
        }
    }
}

pub fn bitmap_file_for_id(filter_id: &str) -> String {
    format!("{EXTRA_DIR}/{filter_id}.roar")
}
