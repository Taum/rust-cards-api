use crate::bitmap::BitmapStore;
use crate::card::parse_card_effects;
use crate::catalog::{Catalog, CatalogBuilder};
use crate::idgd_catalog::IdGdCatalogBuilder;
use crate::crawl::{discover_card_files, DiscoverOptions};
use crate::progress::{BuildProgress, DiscoveryProgress, WriteProgress};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub version: u32,
    pub set: String,
    pub root: String,
    pub built_at_secs: u64,
    pub card_count: u32,
    pub id_gd_count: usize,
    pub total_bit_span: u32,
    pub family_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_limit: Option<usize>,
}

pub struct BuildOptions {
    pub file_limit: Option<usize>,
}

pub fn build(
    dataset_root: &Path,
    set: &str,
    out: &Path,
    options: BuildOptions,
) -> Result<BuildSummary> {
    let limit = options.file_limit;
    let discovery = DiscoveryProgress::start();

    let discovered = discover_card_files(
        dataset_root,
        set,
        DiscoverOptions {
            max_files: limit,
        },
        Some(&discovery),
    )?;

    let files = discovered.files;
    let total_files = files.len();
    discovery.finish(total_files, limit, discovered.stopped_early);

    let progress = BuildProgress::start(total_files);
    let mut catalog_builder = CatalogBuilder::new(set);
    let mut bitmaps = BitmapStore::new();
    let mut idgd_catalog_builder = IdGdCatalogBuilder::new();

    for file in &files {
        let card_index = catalog_builder.on_card(&file.parsed)?;
        let occurrences = parse_card_effects(&file.path)?;
        for occ in &occurrences {
            idgd_catalog_builder.record_first(occ);
            bitmaps.insert(occ.id_gd, card_index);
        }
        progress.inc();
    }

    progress.finish("Indexing complete");

    catalog_builder.finalize_last()?;
    let catalog = catalog_builder.into_catalog()?;

    let write_progress = WriteProgress::start();
    let set_out = out.join(set);
    let id_gd_dir = set_out.join("id_gd");
    fs_create_dir_all(&set_out)?;
    catalog.save(&set_out.join("catalog.json"))?;
    let bitmap_bytes = bitmaps.write_dir(&id_gd_dir)?;
    let idgd_catalog = idgd_catalog_builder.build(set, &bitmaps, &bitmap_bytes);
    IdGdCatalogBuilder::save(&idgd_catalog, &set_out.join("idgd_catalog.json"))?;

    let built_at_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let manifest = Manifest {
        version: 1,
        set: set.to_string(),
        root: dataset_root
            .canonicalize()
            .unwrap_or_else(|_| dataset_root.to_path_buf())
            .display()
            .to_string(),
        built_at_secs,
        card_count: catalog.total_cards_indexed(),
        id_gd_count: bitmaps.len(),
        total_bit_span: catalog.total_bit_span,
        family_count: catalog.families.len(),
        file_limit: limit,
    };
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(set_out.join("manifest.json"), manifest_text)?;
    write_progress.finish();

    Ok(BuildSummary {
        catalog,
        output_dir: set_out,
        files_processed: total_files,
        id_gd_count: manifest.id_gd_count,
        file_limit: limit,
        stopped_early: discovered.stopped_early,
    })
}

pub struct BuildSummary {
    pub catalog: Catalog,
    pub output_dir: std::path::PathBuf,
    pub files_processed: usize,
    pub id_gd_count: usize,
    pub file_limit: Option<usize>,
    pub stopped_early: bool,
}

fn fs_create_dir_all(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}
