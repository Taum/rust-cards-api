use anyhow::{Context, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const EN: &str = "en_US";
pub const FR: &str = "fr_FR";
pub const DE: &str = "de_DE";
pub const ES: &str = "es_ES";
pub const IT: &str = "it_IT";

pub const ALL_LOCALES: [&str; 5] = [EN, FR, DE, ES, IT];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleTier {
    /// English only.
    EnOnly,
    /// English + French.
    EnFr,
    /// All five locales.
    Full,
}

impl LocaleTier {
    pub fn as_str(self) -> &'static str {
        match self {
            LocaleTier::EnOnly => "en_only",
            LocaleTier::EnFr => "en_fr",
            LocaleTier::Full => "full",
        }
    }

    pub fn locales(self) -> Vec<&'static str> {
        match self {
            LocaleTier::EnOnly => vec![EN],
            LocaleTier::EnFr => vec![EN, FR],
            LocaleTier::Full => ALL_LOCALES.to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalePolicy {
    /// Fraction of cards that receive all five locales (includes FR).
    pub full_fraction: f64,
    /// Fraction of cards that receive at least English + French (includes `full_fraction`).
    pub fr_fraction: f64,
}

impl Default for LocalePolicy {
    fn default() -> Self {
        Self {
            full_fraction: 0.01,
            fr_fraction: 0.10,
        }
    }
}

impl LocalePolicy {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.full_fraction >= 0.0 && self.full_fraction <= 1.0,
            "full-locale-fraction must be in [0, 1]"
        );
        anyhow::ensure!(
            self.fr_fraction >= self.full_fraction,
            "fr-locale-fraction ({}) must be >= full-locale-fraction ({})",
            self.fr_fraction,
            self.full_fraction
        );
        anyhow::ensure!(
            self.fr_fraction <= 1.0,
            "fr-locale-fraction must be <= 1"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocaleAvailability {
    pub en: bool,
    pub fr: bool,
    pub de: bool,
    pub es: bool,
    pub it: bool,
}

impl LocaleAvailability {
    pub fn has(&self, locale: &str) -> bool {
        match locale {
            EN => self.en,
            FR => self.fr,
            DE => self.de,
            ES => self.es,
            IT => self.it,
            _ => false,
        }
    }

    pub fn has_all(&self) -> bool {
        self.en && self.fr && self.de && self.es && self.it
    }

    pub fn has_en_fr(&self) -> bool {
        self.en && self.fr
    }

    pub fn locales_present(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.en {
            out.push(EN);
        }
        if self.fr {
            out.push(FR);
        }
        if self.de {
            out.push(DE);
        }
        if self.es {
            out.push(ES);
        }
        if self.it {
            out.push(IT);
        }
        out
    }
}

#[derive(Debug, Deserialize)]
struct CardJsonSlim {
    #[serde(default)]
    #[serde(rename = "imagePath")]
    image_path: Option<String>,
    #[serde(default)]
    translations: BTreeMap<String, TranslationSlim>,
}

#[derive(Debug, Deserialize)]
struct TranslationSlim {
    #[serde(default)]
    image: Option<String>,
}

pub fn card_json_path(
    equinox_root: &Path,
    set: &str,
    faction: &str,
    family_number: &str,
    reference: &str,
) -> PathBuf {
    equinox_root
        .join(format!("cards-unique-{set}"))
        .join("json")
        .join(set)
        .join(faction)
        .join(family_number)
        .join(format!("{reference}.json"))
}

pub fn load_locale_availability(path: &Path) -> Result<LocaleAvailability> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let card: CardJsonSlim = serde_json::from_str(&text)
        .with_context(|| format!("parse JSON {}", path.display()))?;
    Ok(availability_from_card(&card))
}

fn availability_from_card(card: &CardJsonSlim) -> LocaleAvailability {
    LocaleAvailability {
        en: pick_rel_path(card, EN).is_some(),
        fr: pick_rel_path(card, FR).is_some(),
        de: pick_rel_path(card, DE).is_some(),
        es: pick_rel_path(card, ES).is_some(),
        it: pick_rel_path(card, IT).is_some(),
    }
}

pub(crate) fn pick_rel_path(card: &CardJsonSlim, locale: &str) -> Option<String> {
    if let Some(t) = card.translations.get(locale) {
        if let Some(img) = t.image.as_deref().filter(|s| !s.is_empty()) {
            return Some(img.to_string());
        }
    }
    if locale == EN {
        if let Some(p) = card.image_path.as_deref().filter(|s| !s.is_empty()) {
            return Some(crate::url::rel_path_from_dev_url(p).to_string());
        }
    }
    None
}

/// Filter requested locales to those present on the card; `en_US` must be present.
pub fn filter_locales(requested: &[&str], avail: &LocaleAvailability) -> Result<Vec<String>> {
    anyhow::ensure!(avail.en, "missing required locale {EN}");
    let mut out = Vec::with_capacity(requested.len());
    for &loc in requested {
        if avail.has(loc) {
            out.push(loc.to_string());
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct PlannedLocales {
    pub tier: LocaleTier,
    pub locales: Vec<String>,
}

/// Start from maximum available locales, then randomly downgrade cards to hit
/// `full_fraction` / `fr_fraction` targets (cards without `fr_FR` cannot be promoted).
pub fn apply_locale_fractions(
    availabilities: &[LocaleAvailability],
    policy: &LocalePolicy,
    rng: &mut impl Rng,
) -> Vec<PlannedLocales> {
    let n = availabilities.len();
    let n_full_target = ((n as f64) * policy.full_fraction).round() as usize;
    let n_fr_target = ((n as f64) * policy.fr_fraction).round() as usize;

    let mut planned: Vec<PlannedLocales> = availabilities
        .iter()
        .map(|avail| PlannedLocales {
            tier: LocaleTier::EnOnly,
            locales: avail
                .locales_present()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        })
        .collect();

    let mut pool: Vec<usize> = (0..n).collect();
    for i in (1..pool.len()).rev() {
        let j = rng.gen_range(0..=i);
        pool.swap(i, j);
    }

    let mut full_assigned = 0usize;
    for &i in &pool {
        if full_assigned >= n_full_target {
            break;
        }
        if !availabilities[i].has_all() {
            continue;
        }
        planned[i].tier = LocaleTier::Full;
        planned[i].locales = ALL_LOCALES.iter().map(|s| s.to_string()).collect();
        full_assigned += 1;
    }

    let mut fr_assigned = planned
        .iter()
        .filter(|p| p.tier == LocaleTier::Full || p.tier == LocaleTier::EnFr)
        .count();
    for &i in &pool {
        if fr_assigned >= n_fr_target {
            break;
        }
        if planned[i].tier == LocaleTier::Full {
            continue;
        }
        if !availabilities[i].has_en_fr() {
            continue;
        }
        planned[i].tier = LocaleTier::EnFr;
        planned[i].locales = vec![EN.to_string(), FR.to_string()];
        fr_assigned += 1;
    }

    for (plan, avail) in planned.iter_mut().zip(availabilities) {
        if plan.tier == LocaleTier::EnOnly {
            plan.locales = if avail.en {
                vec![EN.to_string()]
            } else {
                plan.locales
                    .iter()
                    .filter(|l| l.as_str() == EN)
                    .cloned()
                    .collect()
            };
        }
    }

    planned
}
