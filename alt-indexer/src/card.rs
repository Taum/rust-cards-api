use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardJson {
    #[serde(default)]
    card_elements: Vec<CardElement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardElement {
    #[serde(default)]
    card_effect_displays: Vec<CardEffectDisplay>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardEffectDisplay {
    #[serde(default)]
    card_effect: Option<CardEffect>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardEffect {
    #[serde(default)]
    card_effect_elements: Vec<CardEffectElement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardEffectElement {
    id_gd: u32,
    #[serde(rename = "type")]
    element_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    translations: Option<BTreeMap<String, LocaleText>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleText {
    pub locale: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct IdGdOccurrence {
    pub id_gd: u32,
    pub element_type: String,
    pub translations: BTreeMap<String, LocaleText>,
}

fn read_card(path: &Path) -> Result<CardJson> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse JSON {}", path.display()))
}

fn translations_from_element(element: &CardEffectElement) -> BTreeMap<String, LocaleText> {
    if let Some(map) = &element.translations {
        return map.clone();
    }
    let mut map = BTreeMap::new();
    if let Some(text) = &element.text {
        map.insert(
            "en_US".to_string(),
            LocaleText {
                locale: "en_US".to_string(),
                text: text.clone(),
            },
        );
    }
    map
}

/// Unique `idGd` occurrences on one card, with effect text metadata.
pub fn parse_card_effects(path: &Path) -> Result<Vec<IdGdOccurrence>> {
    let card = read_card(path)?;
    let mut seen = HashSet::new();
    let mut occurrences = Vec::new();

    for element in &card.card_elements {
        for display in &element.card_effect_displays {
            if let Some(effect) = &display.card_effect {
                for node in &effect.card_effect_elements {
                    if seen.insert(node.id_gd) {
                        occurrences.push(IdGdOccurrence {
                            id_gd: node.id_gd,
                            element_type: node
                                .element_type
                                .clone()
                                .unwrap_or_else(|| "UNKNOWN".to_string()),
                            translations: translations_from_element(node),
                        });
                    }
                }
            }
        }
    }
    Ok(occurrences)
}

/// Unique `idGd` values on one card (deduped per card).
pub fn extract_id_gd(path: &Path) -> Result<Vec<u32>> {
    Ok(parse_card_effects(path)?
        .into_iter()
        .map(|o| o.id_gd)
        .collect())
}
