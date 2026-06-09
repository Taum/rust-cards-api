use crate::catalog::Catalog;
use crate::extra_catalog::{
    bitmap_file_for_id, ExtraCatalog, ExtraCatalogEntry, ExtraFilterType, EXTRA_CATALOG_FILE,
    EXTRA_DIR,
};
use crate::refs_bitmap::{build_bitmap_from_refs_file, validate_bitmap_span};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct IndexManifest {
    set: String,
    total_bit_span: u32,
}

#[derive(Debug, Clone)]
pub struct AddExtraFilterOptions {
    pub index_dir: PathBuf,
    pub filter_id: String,
    pub refs_file: PathBuf,
    pub filter_type: Option<ExtraFilterType>,
    pub negated: bool,
    /// Overwrite an existing filter with the same `--filter-id`.
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddExtraFilterSummary {
    pub filter_id: String,
    pub replaced: bool,
    pub refs_read: usize,
    pub refs_resolved: u64,
    pub negated: bool,
    pub filter_type: Option<ExtraFilterType>,
    pub card_count: u64,
    pub bitmap_bytes: u64,
    pub bitmap_path: PathBuf,
}

pub fn add_extra_filter(opts: &AddExtraFilterOptions) -> Result<AddExtraFilterSummary> {
    validate_filter_id(&opts.filter_id)?;

    let catalog_path = opts.index_dir.join("catalog.json");
    let manifest_path = opts.index_dir.join("manifest.json");
    let catalog = Catalog::load(&catalog_path)?;
    let manifest = load_manifest(&manifest_path)?;

    if catalog.set != manifest.set {
        bail!(
            "catalog set {:?} does not match manifest set {:?}",
            catalog.set,
            manifest.set
        );
    }

    let (refs_read, bitmap) = build_bitmap_from_refs_file(&opts.refs_file, &catalog)?;
    validate_bitmap_span(&bitmap, manifest.total_bit_span)?;
    let card_count = bitmap.len();
    let extra_dir = opts.index_dir.join(EXTRA_DIR);
    fs::create_dir_all(&extra_dir)
        .with_context(|| format!("create {}", extra_dir.display()))?;

    let bitmap_path = extra_dir.join(format!("{}.roar", opts.filter_id));
    let extra_catalog_path = opts.index_dir.join(EXTRA_CATALOG_FILE);
    let catalog_had_id = extra_catalog_path
        .exists()
        .then(|| ExtraCatalog::load(&extra_catalog_path))
        .transpose()?
        .is_some_and(|c| c.contains_id(&opts.filter_id));
    let bitmap_exists = bitmap_path.exists();

    if !opts.replace && (catalog_had_id || bitmap_exists) {
        if catalog_had_id {
            bail!(
                "extra filter id already exists: {} (use --replace to overwrite)",
                opts.filter_id
            );
        }
        bail!(
            "bitmap file already exists: {} (use --replace to overwrite)",
            bitmap_path.display()
        );
    }

    let mut bytes = Vec::new();
    bitmap
        .serialize_into(&mut bytes)
        .context("serialize extra filter bitmap")?;
    let bitmap_bytes = bytes.len() as u64;
    fs::write(&bitmap_path, &bytes)
        .with_context(|| format!("write {}", bitmap_path.display()))?;

    let mut extra_catalog = if extra_catalog_path.exists() {
        let loaded = ExtraCatalog::load(&extra_catalog_path)?;
        if loaded.set != catalog.set {
            bail!(
                "extra_catalog set {:?} does not match index set {:?}",
                loaded.set,
                catalog.set
            );
        }
        loaded
    } else {
        ExtraCatalog::new(&catalog.set)
    };

    let entry = ExtraCatalogEntry {
        id: opts.filter_id.clone(),
        r#type: opts.filter_type,
        negated: opts.negated,
        card_count,
        bitmap_bytes,
        bitmap_file: bitmap_file_for_id(&opts.filter_id),
    };
    let replaced = extra_catalog.upsert_entry(entry);
    extra_catalog.save(&extra_catalog_path)?;

    Ok(AddExtraFilterSummary {
        filter_id: opts.filter_id.clone(),
        replaced,
        refs_read,
        refs_resolved: card_count,
        negated: opts.negated,
        filter_type: opts.filter_type,
        card_count,
        bitmap_bytes,
        bitmap_path,
    })
}

fn load_manifest(path: &Path) -> Result<IndexManifest> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn validate_filter_id(filter_id: &str) -> Result<()> {
    if filter_id.is_empty() {
        bail!("--filter-id must not be empty");
    }
    if !filter_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "--filter-id must contain only ASCII letters, digits, hyphens, and underscores"
        );
    }
    Ok(())
}

