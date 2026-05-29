use std::collections::BTreeMap;
use std::sync::Arc;

use alt_indexer::card::LocaleText;
use alt_indexer::idgd_catalog::{IdGdCatalog, IdGdCatalogEntry};
use anyhow::Context;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::AppState;

/// `GET /api/v2/effects` response body (see `docs/api-spec.md`).
#[derive(Debug, Clone, Serialize)]
pub struct EffectsListResponse {
    pub triggers: Vec<EffectPartWithRegion>,
    pub conditions: Vec<EffectPartWithRegion>,
    pub output: Vec<EffectPart>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPartWithRegion {
    pub id_gd: u32,
    pub text: BTreeMap<String, String>,
    pub is_echo: bool,
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPart {
    pub id_gd: u32,
    pub text: BTreeMap<String, String>,
}

/// Build the effects list from `idgd_catalog.json` entries.
pub fn build_effects_list(catalog: &IdGdCatalog) -> EffectsListResponse {
    let mut triggers = Vec::new();
    let mut conditions = Vec::new();
    let mut output = Vec::new();

    for entry in &catalog.entries {
        match entry.element_type.as_str() {
            "TRIGGER" => triggers.push(effect_part_with_region(entry)),
            "CONDITION" => conditions.push(effect_part_with_region(entry)),
            "OUTPUT" => output.push(effect_part(entry)),
            _ => {}
        }
    }

    triggers.sort_by_key(|e| e.id_gd);
    conditions.sort_by_key(|e| e.id_gd);
    output.sort_by_key(|e| e.id_gd);

    EffectsListResponse {
        triggers,
        conditions,
        output,
    }
}

/// Serialize [`EffectsListResponse`] once at startup for a static endpoint body.
pub fn serialize_effects_list(response: &EffectsListResponse) -> anyhow::Result<Bytes> {
    let bytes = serde_json::to_vec(response).context("serialize effects list")?;
    Ok(Bytes::from(bytes))
}

fn effect_part_with_region(entry: &IdGdCatalogEntry) -> EffectPartWithRegion {
    EffectPartWithRegion {
        id_gd: entry.id_gd,
        text: translations_to_text(&entry.translations),
        is_echo: entry.is_echo,
        is_main: entry.is_main,
    }
}

fn effect_part(entry: &IdGdCatalogEntry) -> EffectPart {
    EffectPart {
        id_gd: entry.id_gd,
        text: translations_to_text(&entry.translations),
    }
}

fn translations_to_text(translations: &BTreeMap<String, LocaleText>) -> BTreeMap<String, String> {
    translations
        .iter()
        .map(|(locale, t)| (locale.clone(), t.text.clone()))
        .collect()
}

pub async fn get_effects_v2(State(state): State<Arc<AppState>>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        state.effects_body().as_ref().clone(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alt_indexer::idgd_catalog::IdGdCatalogEntry;

    fn locale_text(locale: &str, text: &str) -> LocaleText {
        LocaleText {
            locale: locale.to_string(),
            text: text.to_string(),
        }
    }

    fn entry(
        id_gd: u32,
        element_type: &str,
        en: &str,
        is_main: bool,
        is_echo: bool,
    ) -> IdGdCatalogEntry {
        IdGdCatalogEntry {
            id_gd,
            card_count: 1,
            bitmap_bytes: 1,
            bitmap_file: format!("{id_gd}.roar"),
            element_type: element_type.to_string(),
            translations: BTreeMap::from([(
                "en_US".to_string(),
                locale_text("en_US", en),
            )]),
            m1: None,
            m2: None,
            m3: None,
            ec: None,
            is_main,
            is_echo,
        }
    }

    #[test]
    fn build_groups_by_element_type_and_sorts_by_id() {
        let catalog = IdGdCatalog {
            set: "TEST".to_string(),
            entries: vec![
                entry(10, "OUTPUT", "out", false, false),
                entry(3, "TRIGGER", "tri", true, false),
                entry(7, "CONDITION", "cond", true, true),
            ],
        };

        let list = build_effects_list(&catalog);
        assert_eq!(list.triggers.len(), 1);
        assert_eq!(list.triggers[0].id_gd, 3);
        assert_eq!(list.triggers[0].text.get("en_US").map(String::as_str), Some("tri"));
        assert!(list.triggers[0].is_main);
        assert!(!list.triggers[0].is_echo);

        assert_eq!(list.conditions[0].id_gd, 7);
        assert!(list.conditions[0].is_echo);

        assert_eq!(list.output[0].id_gd, 10);
        assert_eq!(list.output.len(), 1);
    }

    #[test]
    fn serialized_body_is_stable_json() {
        let catalog = IdGdCatalog {
            set: "TEST".to_string(),
            entries: vec![entry(1, "TRIGGER", "{R}", true, false)],
        };
        let list = build_effects_list(&catalog);
        let body = serialize_effects_list(&list).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["triggers"][0]["idGd"], 1);
        assert_eq!(value["triggers"][0]["text"]["en_US"], "{R}");
        assert_eq!(value["triggers"][0]["isMain"], true);
        assert_eq!(value["conditions"].as_array().unwrap().len(), 0);
        assert_eq!(value["output"].as_array().unwrap().len(), 0);
    }
}
