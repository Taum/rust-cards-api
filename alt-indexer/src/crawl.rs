use crate::path::{is_foiler_path, parse_card_path, sort_key, ParsedCardPath};
use crate::progress::DiscoveryProgress;
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct DiscoverOptions {
    /// Stop discovering (and indexing) after this many card files. For testing.
    pub max_files: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct DiscoverResult {
    pub files: Vec<CardFile>,
    pub limit: Option<usize>,
    /// True when `max_files` was set and discovery stopped before scanning the full tree.
    pub stopped_early: bool,
}

#[derive(Debug, Clone)]
pub struct CardFile {
    pub path: PathBuf,
    pub parsed: ParsedCardPath,
}

/// Discover card JSON paths under `json/<set>/`, skip FOILER, sort for catalog order.
pub fn discover_card_files(
    dataset_root: &Path,
    set: &str,
    options: DiscoverOptions,
    progress: Option<&DiscoveryProgress>,
) -> Result<DiscoverResult> {
    let set_root = dataset_root.join("json").join(set);
    if !set_root.is_dir() {
        anyhow::bail!(
            "set directory not found: {} (expected json/{set}/ under root)",
            set_root.display()
        );
    }

    let limit = options.max_files;
    let mut files = Vec::new();
    let mut stopped_early = false;

    'walk: for entry in WalkDir::new(&set_root).into_iter().filter_map(|e| e.ok()) {
        if let Some(max) = limit {
            if files.len() >= max {
                stopped_early = true;
                break 'walk;
            }
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if is_foiler_path(path) {
            continue;
        }
        let parsed = match parse_card_path(path, set) {
            Ok(p) => p,
            Err(_) => continue,
        };
        files.push(CardFile {
            path: path.to_path_buf(),
            parsed,
        });

        if let Some(p) = progress {
            let n = files.len();
            let last_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            p.set_found(n, limit, last_name);
        }

        if let Some(max) = limit {
            if files.len() >= max {
                stopped_early = true;
                break 'walk;
            }
        }
    }

    files.sort_by(|a, b| sort_key(&a.parsed).cmp(&sort_key(&b.parsed)));

    Ok(DiscoverResult {
        files,
        limit,
        stopped_early,
    })
}
