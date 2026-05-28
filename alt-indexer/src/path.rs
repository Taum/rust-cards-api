use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FACTION_ORDER: [&str; 6] = ["AX", "BR", "LY", "MU", "OR", "YZ"];

static CARD_FILE_RE: OnceLock<regex::Regex> = OnceLock::new();

fn card_file_re() -> &'static regex::Regex {
    CARD_FILE_RE.get_or_init(|| {
        regex::Regex::new(
            r"^ALT_(?P<set>[^_]+(?:_[^_]+)*)_B_(?P<faction>[A-Z]{2})_(?P<family>\d+)_U_(?P<uid>\d+)\.json$",
        )
        .expect("card file regex")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCardPath {
    pub set: String,
    pub faction: String,
    pub family_number: String,
    pub unique_id: u32,
}

impl ParsedCardPath {
    pub fn family_id(&self) -> String {
        format!("{}_{}", self.faction, self.family_number)
    }

    pub fn reference(&self) -> String {
        format!(
            "ALT_{}_B_{}_{}_U_{}",
            self.set, self.faction, self.family_number, self.unique_id
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CardPathSortKey {
    pub faction_rank: u8,
    pub family_number: u32,
    pub unique_id: u32,
}

pub fn faction_rank(faction: &str) -> u8 {
    FACTION_ORDER
        .iter()
        .position(|f| *f == faction)
        .unwrap_or(255) as u8
}

pub fn sort_key(parsed: &ParsedCardPath) -> CardPathSortKey {
    CardPathSortKey {
        faction_rank: faction_rank(&parsed.faction),
        family_number: parsed.family_number.parse().unwrap_or(u32::MAX),
        unique_id: parsed.unique_id,
    }
}

pub fn is_foiler_path(path: &Path) -> bool {
    path.to_string_lossy().contains("FOILER")
}

/// Parse a card JSON path: `json/<SET>/<faction>/<familyNumber>/<file>.json`
pub fn parse_card_path(path: &Path, expected_set: &str) -> Result<ParsedCardPath> {
    if is_foiler_path(path) {
        bail!("foiler path is not indexed: {}", path.display());
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("path has no file name")?;

    let caps = card_file_re()
        .captures(file_name)
        .with_context(|| format!("file name does not match card pattern: {file_name}"))?;

    let set = caps.name("set").unwrap().as_str().to_string();
    if set != expected_set {
        bail!(
            "set mismatch: path has {set}, expected {expected_set} ({})",
            path.display()
        );
    }

    let faction = caps.name("faction").unwrap().as_str().to_string();
    let family_number = caps.name("family").unwrap().as_str().to_string();
    let unique_id: u32 = caps
        .name("uid")
        .unwrap()
        .as_str()
        .parse()
        .context("invalid UniqueID")?;

    let components: Vec<_> = path.components().collect();
    let json_idx = components
        .iter()
        .position(|c| c.as_os_str() == "json")
        .context("path missing json/ segment")?;
    let set_in_path = components
        .get(json_idx + 1)
        .and_then(|c| c.as_os_str().to_str())
        .context("path missing set segment after json/")?;
    if set_in_path != expected_set {
        bail!("set in path ({set_in_path}) does not match file name ({set})");
    }
    let path_faction = components
        .get(json_idx + 2)
        .and_then(|c| c.as_os_str().to_str())
        .context("path missing faction segment")?;
    let path_family = components
        .get(json_idx + 3)
        .and_then(|c| c.as_os_str().to_str())
        .context("path missing familyNumber segment")?;

    if path_faction != faction || path_family != family_number {
        bail!(
            "path segments ({path_faction}/{path_family}) do not match file name ({faction}/{family_number})"
        );
    }

    Ok(ParsedCardPath {
        set,
        faction,
        family_number,
        unique_id,
    })
}

pub fn json_set_root(dataset_root: &Path, set: &str) -> PathBuf {
    dataset_root.join("json").join(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tmp_example() {
        let path = Path::new("json/COREKS/AX/06/ALT_COREKS_B_AX_06_U_5.json");
        let parsed = parse_card_path(path, "COREKS").unwrap();
        assert_eq!(parsed.faction, "AX");
        assert_eq!(parsed.family_number, "06");
        assert_eq!(parsed.unique_id, 5);
        assert_eq!(parsed.reference(), "ALT_COREKS_B_AX_06_U_5");
    }

    #[test]
    fn parse_three_digit_family() {
        let path = Path::new("json/COREKS/AX/101/ALT_COREKS_B_AX_101_U_5.json");
        let parsed = parse_card_path(path, "COREKS").unwrap();
        assert_eq!(parsed.faction, "AX");
        assert_eq!(parsed.family_number, "101");
        assert_eq!(parsed.unique_id, 5);
        assert_eq!(parsed.reference(), "ALT_COREKS_B_AX_101_U_5");
    }

    #[test]
    fn foiler_rejected() {
        let path = Path::new("json/COREKS/NE/00/ALT_COREKS_B_NE_FOILER_U.json");
        assert!(parse_card_path(path, "COREKS").is_err());
    }
}
