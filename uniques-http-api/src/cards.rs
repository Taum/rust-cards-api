use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use roaring::RoaringBitmap;
use serde::Serialize;

use alt_indexer::bitmap::EffectLine;

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct CardsIter {
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CardRef {
    pub reference: String,
}

#[derive(Debug, Serialize)]
pub struct CardsResponse {
    pub iter: CardsIter,
    pub cards: Vec<CardRef>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

#[derive(Debug, Clone, Copy)]
enum IdGdSelector {
    T,
    C,
    O,
}

impl IdGdSelector {
    fn expected_type(self) -> &'static str {
        match self {
            IdGdSelector::T => "TRIGGER",
            IdGdSelector::C => "CONDITION",
            IdGdSelector::O => "OUTPUT",
        }
    }
}

#[derive(Debug, Default)]
struct AbilityFilters {
    effect0_t: Vec<u32>,
    effect0_c: Vec<u32>,
    effect0_o: Vec<u32>,
    support_t: Vec<u32>,
    support_c: Vec<u32>,
    support_o: Vec<u32>,
}

#[derive(Debug)]
struct CardsRequest {
    limit: usize,
    cursor: Option<u32>,
    filters: AbilityFilters,
}

pub async fn get_cards_v2(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<CardsResponse>> {
    let req = parse_request(&state, &params)?;
    let bitmap = build_bitmap(&state, &req.filters)?;
    let total = bitmap.len() as u64;
    let (cards, next_cursor) = page_references(&state, &bitmap, req.cursor, req.limit)?;

    Ok(Json(CardsResponse {
        iter: CardsIter {
            total,
            cursor: next_cursor,
        },
        cards,
    }))
}

fn parse_request(state: &AppState, params: &HashMap<String, String>) -> ApiResult<CardsRequest> {
    // Reject any unsupported effect predicates beyond effect[0]
    for key in params.keys() {
        if key.starts_with("effect[") && !key.starts_with("effect[0]") {
            return Err(bad_request(format!(
                "unsupported parameter '{key}': only effect[0] is supported in this iteration"
            )));
        }
    }

    let limit = match params.get("limit") {
        None => 50usize,
        Some(v) => parse_usize("limit", v)?,
    };
    if !(1..=200).contains(&limit) {
        return Err(bad_request("limit must be in range 1..=200".to_string()));
    }

    let cursor = match params.get("cursor") {
        None => None,
        Some(v) => Some(parse_u32("cursor", v)?),
    };
    if let Some(c) = cursor {
        let total_bit_span = state.manifest().total_bit_span;
        if c >= total_bit_span {
            return Err(bad_request(format!(
                "cursor must be < total_bit_span ({total_bit_span})"
            )));
        }
    }

    let mut filters = AbilityFilters::default();
    filters.effect0_t = parse_id_list(params, "effect[0][t]")?;
    filters.effect0_c = parse_id_list(params, "effect[0][c]")?;
    filters.effect0_o = parse_id_list(params, "effect[0][o]")?;
    filters.support_t = parse_id_list(params, "support[t]")?;
    filters.support_c = parse_id_list(params, "support[c]")?;
    filters.support_o = parse_id_list(params, "support[o]")?;

    // Must specify at least one predicate.
    let has_any = !filters.effect0_t.is_empty()
        || !filters.effect0_c.is_empty()
        || !filters.effect0_o.is_empty()
        || !filters.support_t.is_empty()
        || !filters.support_c.is_empty()
        || !filters.support_o.is_empty();
    if !has_any {
        return Err(bad_request(
            "must provide at least one of effect[0][t|c|o] or support[t|c|o]".to_string(),
        ));
    }

    validate_idgd_types(state, &filters)?;

    Ok(CardsRequest {
        limit,
        cursor,
        filters,
    })
}

fn parse_id_list(params: &HashMap<String, String>, key: &str) -> ApiResult<Vec<u32>> {
    let Some(v) = params.get(key) else {
        return Ok(Vec::new());
    };
    if v.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in v.split(',') {
        let s = part.trim();
        if s.is_empty() {
            continue;
        }
        out.push(parse_u32(key, s)?);
    }
    Ok(out)
}

fn validate_idgd_types(state: &AppState, filters: &AbilityFilters) -> ApiResult<()> {
    let mut types: BTreeMap<u32, &str> = BTreeMap::new();
    for entry in &state.idgd_catalog().entries {
        types.insert(entry.id_gd, entry.element_type.as_str());
    }

    for (key, selector, ids) in [
        ("effect[0][t]", IdGdSelector::T, &filters.effect0_t),
        ("effect[0][c]", IdGdSelector::C, &filters.effect0_c),
        ("effect[0][o]", IdGdSelector::O, &filters.effect0_o),
        ("support[t]", IdGdSelector::T, &filters.support_t),
        ("support[c]", IdGdSelector::C, &filters.support_c),
        ("support[o]", IdGdSelector::O, &filters.support_o),
    ] {
        for &id in ids.iter() {
            let Some(actual) = types.get(&id).copied() else {
                return Err(bad_request(format!(
                    "{key} contains unknown idGd {id} (not present in idgd_catalog)"
                )));
            };
            let expected = selector.expected_type();
            if actual != expected {
                return Err(bad_request(format!(
                    "{key} contains idGd {id} of type {actual}, expected {expected}"
                )));
            }
        }
    }

    Ok(())
}

fn build_bitmap(state: &AppState, filters: &AbilityFilters) -> ApiResult<RoaringBitmap> {
    let mut groups = Vec::new();

    // effect[0] searches main lines only (M1/M2/M3) with per-line bucket intersection,
    // then OR across lines. This matches alt-indexer's default per-line query behavior.
    let effect0 = effect0_bitmap_main_lines(
        state,
        &filters.effect0_t,
        &filters.effect0_c,
        &filters.effect0_o,
    );
    if let Some(bmp) = effect0 {
        groups.push(bmp);
    }

    // support[...] searches echo/support line only (Ec), with per-line bucket intersection.
    let support = bitmap_intersect_buckets_on_line(
        state,
        EffectLine::Ec,
        &filters.support_t,
        &filters.support_c,
        &filters.support_o,
    );
    if let Some(bmp) = support {
        groups.push(bmp);
    }

    let mut it = groups.into_iter();
    let mut out = match it.next() {
        Some(first) => first,
        None => RoaringBitmap::new(),
    };
    for bmp in it {
        out &= bmp;
    }
    Ok(out)
}

fn effect0_bitmap_main_lines(
    state: &AppState,
    triggers: &[u32],
    conditions: &[u32],
    outputs: &[u32],
) -> Option<RoaringBitmap> {
    if triggers.is_empty() && conditions.is_empty() && outputs.is_empty() {
        return None;
    }

    let mut out = RoaringBitmap::new();
    for line in [EffectLine::M1, EffectLine::M2, EffectLine::M3] {
        if let Some(line_match) =
            bitmap_intersect_buckets_on_line(state, line, triggers, conditions, outputs)
        {
            out |= line_match;
        }
    }
    Some(out)
}

fn bitmap_intersect_buckets_on_line(
    state: &AppState,
    line: EffectLine,
    triggers: &[u32],
    conditions: &[u32],
    outputs: &[u32],
) -> Option<RoaringBitmap> {
    let mut groups: Vec<RoaringBitmap> = Vec::new();
    if !triggers.is_empty() {
        groups.push(bitmap_line_any_ids(state, line, triggers));
    }
    if !conditions.is_empty() {
        groups.push(bitmap_line_any_ids(state, line, conditions));
    }
    if !outputs.is_empty() {
        groups.push(bitmap_line_any_ids(state, line, outputs));
    }
    let mut it = groups.into_iter();
    let mut bmp = it.next()?;
    for g in it {
        bmp &= g;
    }
    Some(bmp)
}

fn bitmap_line_any_ids(state: &AppState, line: EffectLine, ids: &[u32]) -> RoaringBitmap {
    let mut out = RoaringBitmap::new();
    for &id in ids {
        out |= bitmap_line(state, line, id);
    }
    out
}

fn bitmap_line(state: &AppState, line: EffectLine, id_gd: u32) -> RoaringBitmap {
    state
        .id_gd_per_line()
        .get(&(id_gd, line))
        .cloned()
        .unwrap_or_else(RoaringBitmap::new)
}

fn page_references(
    state: &AppState,
    bitmap: &RoaringBitmap,
    cursor: Option<u32>,
    limit: usize,
) -> ApiResult<(Vec<CardRef>, Option<u32>)> {
    let mut out = Vec::with_capacity(limit);
    let mut last_index: Option<u32> = None;

    for card_index in bitmap.iter() {
        if cursor.is_some_and(|c| card_index <= c) {
            continue;
        }
        let reference = state
            .decode_reference(card_index)
            .map_err(|e| bad_request(format!("failed to decode reference for card_index {card_index}: {e}")))?;
        out.push(CardRef { reference });
        last_index = Some(card_index);
        if out.len() >= limit {
            break;
        }
    }

    // Only return a cursor when there may be another page.
    let next_cursor = if out.len() == limit { last_index } else { None };

    Ok((out, next_cursor))
}

fn parse_u32(field: &str, s: &str) -> ApiResult<u32> {
    s.parse::<u32>()
        .map_err(|_| bad_request(format!("invalid {field} value '{s}'")))
}

fn parse_usize(field: &str, s: &str) -> ApiResult<usize> {
    s.parse::<usize>()
        .map_err(|_| bad_request(format!("invalid {field} value '{s}'")))
}

fn bad_request(msg: String) -> (StatusCode, Json<ApiError>) {
    (StatusCode::BAD_REQUEST, Json(ApiError { error: msg }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use alt_indexer::catalog::{Catalog, FamilyEntry, FACTION_ORDER};
    use alt_indexer::faction_index::Faction;
    use alt_indexer::idgd_catalog::{IdGdCatalog, IdGdCatalogEntry};
    use alt_indexer::stat_index::StatField;

    use crate::loader::{FactionsSummary, IndexManifest, StatsSummary};
    use crate::state::AppStateInner;

    fn test_state() -> AppState {
        let catalog = Catalog {
            set: "TEST".to_string(),
            faction_order: FACTION_ORDER.iter().map(|s| s.to_string()).collect(),
            families: vec![FamilyEntry {
                start_bit: 0,
                faction: "AX".to_string(),
                family_number: "01".to_string(),
                family_id: "AX_01".to_string(),
                source_set: None,
                max_unique_id: 10,
                card_count: 10,
                first_reference: "ALT_TEST_B_AX_01_U_1".to_string(),
            }],
            total_bit_span: 10,
        };

        let manifest = IndexManifest {
            version: 1,
            set: "TEST".to_string(),
            kind: None,
            built_at_secs: 0,
            card_count: 10,
            id_gd_count: 3,
            total_bit_span: 10,
            family_count: 1,
        };

        let idgd_catalog = IdGdCatalog {
            set: "TEST".to_string(),
            entries: vec![
                IdGdCatalogEntry {
                    id_gd: 24,
                    card_count: 2,
                    bitmap_bytes: 0,
                    bitmap_file: "24.roar".to_string(),
                    element_type: "TRIGGER".to_string(),
                    translations: BTreeMap::new(),
                    m1: None,
                    m2: None,
                    m3: None,
                    ec: None,
                    is_echo: Some(false),
                },
                IdGdCatalogEntry {
                    id_gd: 191,
                    card_count: 1,
                    bitmap_bytes: 0,
                    bitmap_file: "191.roar".to_string(),
                    element_type: "CONDITION".to_string(),
                    translations: BTreeMap::new(),
                    m1: None,
                    m2: None,
                    m3: None,
                    ec: None,
                    is_echo: Some(false),
                },
                IdGdCatalogEntry {
                    id_gd: 42,
                    card_count: 1,
                    bitmap_bytes: 0,
                    bitmap_file: "42.roar".to_string(),
                    element_type: "OUTPUT".to_string(),
                    translations: BTreeMap::new(),
                    m1: None,
                    m2: None,
                    m3: None,
                    ec: None,
                    is_echo: Some(true),
                },
            ],
        };

        let stats_summary = StatsSummary {
            version: 1,
            set: "TEST".to_string(),
            total_cards_indexed: 10,
            fields: vec![],
        };
        let factions_summary = FactionsSummary {
            version: 1,
            set: "TEST".to_string(),
            total_cards_indexed: 10,
            source: "test".to_string(),
            factions: vec![],
            unknown_count: 0,
            bitmap_dir: "factions".to_string(),
        };

        let mut id_gd_per_line: BTreeMap<(u32, EffectLine), RoaringBitmap> = BTreeMap::new();
        id_gd_per_line.insert((24, EffectLine::M2), RoaringBitmap::from_iter([2, 5]));
        id_gd_per_line.insert((191, EffectLine::M1), RoaringBitmap::from_iter([5]));
        id_gd_per_line.insert((42, EffectLine::Ec), RoaringBitmap::from_iter([5]));

        let inner = AppStateInner {
            index_dir: "C:\\tmp\\index".into(),
            catalog,
            manifest,
            idgd_catalog,
            stats_summary,
            factions_summary,
            cards: vec![0u8; 10 * alt_indexer::compact::RECORD_SIZE],
            id_gd_whole: BTreeMap::new(),
            id_gd_per_line,
            stats: BTreeMap::<StatField, [RoaringBitmap; 16]>::new(),
            factions: BTreeMap::<Faction, RoaringBitmap>::new(),
        };

        AppState::new(Arc::new(inner))
    }

    #[test]
    fn type_validation_rejects_wrong_kind() {
        let state = test_state();
        let mut params = HashMap::new();
        params.insert("effect[0][t]".to_string(), "191".to_string()); // CONDITION but using [t]
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn effect0_matches_any_main_line() {
        let state = test_state();
        let mut params = HashMap::new();
        params.insert("effect[0][t]".to_string(), "24".to_string());
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req.filters).unwrap();
        assert!(bmp.contains(2));
        assert!(bmp.contains(5));
        assert_eq!(bmp.len(), 2);
    }

    #[test]
    fn effect0_intersects_across_selectors_on_same_line() {
        let state = test_state();
        // trigger 24 exists on M2 for {2,5}; output 42 exists only on Ec for {5}.
        // Since effect[0] searches M1/M2/M3 and requires same-line intersection, there are no matches.
        let mut params = HashMap::new();
        params.insert("effect[0][t]".to_string(), "24".to_string());
        params.insert("effect[0][o]".to_string(), "42".to_string());
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req.filters).unwrap();
        assert!(bmp.is_empty());
    }

    #[test]
    fn effect_and_support_intersect() {
        let state = test_state();
        let mut params = HashMap::new();
        params.insert("effect[0][t]".to_string(), "24".to_string());
        params.insert("support[o]".to_string(), "42".to_string());
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req.filters).unwrap();
        assert!(!bmp.contains(2));
        assert!(bmp.contains(5));
        assert_eq!(bmp.len(), 1);
    }

    #[test]
    fn paging_uses_raw_card_index_cursor() {
        let state = test_state();
        let mut params = HashMap::new();
        params.insert("effect[0][t]".to_string(), "24".to_string());
        params.insert("limit".to_string(), "1".to_string());
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req.filters).unwrap();

        let (page1, cur1) = page_references(&state, &bmp, None, 1).unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].reference, "ALT_TEST_B_AX_01_U_3"); // card_index 2 => unique_id 3
        assert_eq!(cur1, Some(2));

        let (page2, cur2) = page_references(&state, &bmp, cur1, 1).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].reference, "ALT_TEST_B_AX_01_U_6"); // card_index 5 => unique_id 6
        assert_eq!(cur2, Some(5));

        let (page3, cur3) = page_references(&state, &bmp, cur2, 1).unwrap();
        assert!(page3.is_empty());
        assert_eq!(cur3, None);
    }
}

