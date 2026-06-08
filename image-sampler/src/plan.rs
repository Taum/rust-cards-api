use crate::locale::LocaleTier;
use serde::{Deserialize, Serialize};

/// Card identity fields shared across plan / resolved / download index rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardIdentity {
    #[serde(rename = "ref")]
    pub reference: String,
    pub set: String,
    pub faction: String,
    pub family_id: String,
    pub family_number: String,
    pub unique_id: u32,
    pub shape: String,
    pub combo_id: String,
}

/// One row in `plan.jsonl`: one sampled card and the locales to download for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCard {
    #[serde(flatten)]
    pub card: CardIdentity,
    pub locale_tier: LocaleTier,
    /// Always includes `en_US` when the card is kept; additional locales are opportunistic
    /// then trimmed to match `locale_tier` / policy fractions.
    pub locales: Vec<String>,
    /// Phase-1 shape-floor pick (one card per `(family_id, shape)` with a unique strict tuple).
    #[serde(default)]
    pub shape_floor: bool,
}

/// One row in `plan-resolved.jsonl`: one card with per-locale `Art/...` paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCard {
    #[serde(flatten)]
    pub card: CardIdentity,
    pub locale_tier: LocaleTier,
    #[serde(default)]
    pub shape_floor: bool,
    /// Locale → relative image path (e.g. `Art/EOLE/CARDS/.../en_US/hash.jpg`).
    /// Always contains `en_US` on success. Download rebuilds prod/proxy URLs from these.
    pub locales: std::collections::BTreeMap<String, String>,
}

/// Flattened download unit derived from a `ResolvedCard` locale entry.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub card: CardIdentity,
    pub locale: String,
    pub locale_tier: LocaleTier,
    pub shape_floor: bool,
    pub rel_path: String,
    pub src_url: String,
}

impl ResolvedCard {
    pub fn download_tasks(
        &self,
        use_proxy: bool,
        proxy_width: u32,
        proxy_quality: u32,
    ) -> Vec<DownloadTask> {
        let mut tasks = Vec::with_capacity(self.locales.len());
        for (locale, rel_path) in &self.locales {
            tasks.push(DownloadTask {
                card: self.card.clone(),
                locale: locale.clone(),
                locale_tier: self.locale_tier,
                shape_floor: self.shape_floor,
                rel_path: rel_path.clone(),
                src_url: crate::url::fetch_url_for_rel_path(
                    rel_path,
                    use_proxy,
                    proxy_width,
                    proxy_quality,
                ),
            });
        }
        tasks
    }
}

/// One row in `images/index.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadIndexRow {
    #[serde(flatten)]
    pub card: CardIdentity,
    pub locale: String,
    pub locale_tier: LocaleTier,
    #[serde(default)]
    pub shape_floor: bool,
    pub rel_path: String,
    pub src_url: String,
    pub local_path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// One row in `errors.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadErrorRow {
    #[serde(flatten)]
    pub card: CardIdentity,
    pub locale: String,
    pub src_url: String,
    pub error: String,
}
