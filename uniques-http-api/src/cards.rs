use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::Json;
use roaring::RoaringBitmap;
use serde::Serialize;
use url::form_urlencoded;

use alt_indexer::bitmap::EffectLine;
use alt_indexer::faction_index::Faction;
use alt_indexer::idgd_catalog::IdGdCatalogEntry;
use alt_indexer::stat_index::StatField;

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct CardsIter {
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CardFaction {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardV2 {
    pub reference: String,
    pub main_cost: u8,
    pub recall_cost: u8,
    pub forest_power: u8,
    pub mountain_power: u8,
    pub ocean_power: u8,
    pub faction: CardFaction,
    pub main_effect: BTreeMap<String, String>,
    pub echo_effect: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct CardsResponse {
    pub iter: CardsIter,
    pub cards: Vec<CardV2>,
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

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone)]
enum CostPredicate {
    Exact(u8),
    AnyOf(Vec<u8>),
    Range { op: CompareOp, value: u8 },
}

#[derive(Debug)]
struct CardsRequest {
    limit: usize,
    cursor: Option<u32>,
    filters: AbilityFilters,
    factions: Vec<Faction>,
    main_cost: Option<CostPredicate>,
    recall_cost: Option<CostPredicate>,
}

pub async fn get_cards_v2(
    State(state): State<Arc<AppState>>,
    RawQuery(query): RawQuery,
) -> ApiResult<Json<CardsResponse>> {
    let params = parse_query_multimap(query.as_deref())?;
    let req = parse_request(&state, &params)?;
    let bitmap = build_bitmap(&state, &req)?;
    let total = bitmap.len() as u64;
    let (cards, next_cursor) = page_cards_v2(&state, &bitmap, req.cursor, req.limit)?;

    Ok(Json(CardsResponse {
        iter: CardsIter {
            total,
            cursor: next_cursor,
        },
        cards,
    }))
}

type QueryMultiMap = HashMap<String, Vec<String>>;

fn parse_query_multimap(query: Option<&str>) -> ApiResult<QueryMultiMap> {
    let mut out: QueryMultiMap = HashMap::new();
    let Some(query) = query else {
        return Ok(out);
    };

    for (k, v) in form_urlencoded::parse(query.as_bytes()) {
        out.entry(k.into_owned())
            .or_insert_with(Vec::new)
            .push(v.into_owned());
    }
    Ok(out)
}

fn get_first<'a>(params: &'a QueryMultiMap, key: &str) -> Option<&'a str> {
    params.get(key)?.first().map(|s| s.as_str())
}

fn has_any(params: &QueryMultiMap, key: &str) -> bool {
    params.get(key).is_some_and(|v| v.iter().any(|s| !s.trim().is_empty()))
}

fn parse_request(state: &AppState, params: &QueryMultiMap) -> ApiResult<CardsRequest> {
    // Reject any unsupported effect predicates beyond effect[0]
    for key in params.keys() {
        if key.starts_with("effect[") && !key.starts_with("effect[0]") {
            return Err(bad_request(format!(
                "unsupported parameter '{key}': only effect[0] is supported in this iteration"
            )));
        }
    }

    let limit = match get_first(params, "limit") {
        None => 50usize,
        Some(v) => parse_usize("limit", v)?,
    };
    if !(1..=200).contains(&limit) {
        return Err(bad_request("limit must be in range 1..=200".to_string()));
    }

    let cursor = match get_first(params, "cursor") {
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

    let factions = parse_factions(params)?;
    let main_cost = parse_cost_predicate(params, "mainCost")?;
    let recall_cost = parse_cost_predicate(params, "recallCost")?;

    // Must specify at least one predicate.
    let has_any = !filters.effect0_t.is_empty()
        || !filters.effect0_c.is_empty()
        || !filters.effect0_o.is_empty()
        || !filters.support_t.is_empty()
        || !filters.support_c.is_empty()
        || !filters.support_o.is_empty()
        || !factions.is_empty()
        || main_cost.is_some()
        || recall_cost.is_some();
    if !has_any {
        return Err(bad_request(
            "must provide at least one filter predicate".to_string(),
        ));
    }

    validate_idgd_types(state, &filters)?;

    Ok(CardsRequest {
        limit,
        cursor,
        filters,
        factions,
        main_cost,
        recall_cost,
    })
}

fn parse_id_list(params: &QueryMultiMap, key: &str) -> ApiResult<Vec<u32>> {
    let Some(values) = params.get(key) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for v in values {
        if v.trim().is_empty() {
            continue;
        }
        for part in v.split(',') {
            let s = part.trim();
            if s.is_empty() {
                continue;
            }
            out.push(parse_u32(key, s)?);
        }
    }
    Ok(out)
}

fn parse_factions(params: &QueryMultiMap) -> ApiResult<Vec<Faction>> {
    // Spec: faction[] repeated keys
    // Convenience: faction=AX,BR
    let mut codes: Vec<String> = Vec::new();
    if let Some(values) = params.get("faction[]") {
        for v in values {
            for part in v.split(',') {
                let s = part.trim();
                if !s.is_empty() {
                    codes.push(s.to_string());
                }
            }
        }
    }
    if let Some(values) = params.get("faction") {
        for v in values {
            for part in v.split(',') {
                let s = part.trim();
                if !s.is_empty() {
                    codes.push(s.to_string());
                }
            }
        }
    }

    let mut out = Vec::new();
    for code in codes {
        let faction = match code.as_str() {
            "AX" => Faction::Ax,
            "BR" => Faction::Br,
            "LY" => Faction::Ly,
            "MU" => Faction::Mu,
            "OR" => Faction::Or,
            "YZ" => Faction::Yz,
            _ => return Err(bad_request(format!("invalid faction value '{code}'"))),
        };
        if !out.contains(&faction) {
            out.push(faction);
        }
    }
    Ok(out)
}

fn parse_cost_u8(field: &str, s: &str) -> ApiResult<u8> {
    let v = s
        .parse::<u8>()
        .map_err(|_| bad_request(format!("invalid {field} value '{s}'")))?;
    if v > 15 {
        return Err(bad_request(format!(
            "invalid {field} value '{s}': must be in range 0..=15"
        )));
    }
    Ok(v)
}

fn parse_cost_array(params: &QueryMultiMap, key: &str) -> ApiResult<Option<Vec<u8>>> {
    let Some(values) = params.get(key) else {
        return Ok(None);
    };
    let mut out: Vec<u8> = Vec::new();
    for v in values {
        if v.trim().is_empty() {
            continue;
        }
        for part in v.split(',') {
            let s = part.trim();
            if s.is_empty() {
                continue;
            }
            let n = parse_cost_u8(key, s)?;
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    if out.is_empty() {
        Ok(None)
    } else {
        Ok(Some(out))
    }
}

fn parse_cost_predicate(params: &QueryMultiMap, base: &str) -> ApiResult<Option<CostPredicate>> {
    let exact_key = base;
    let array_key = format!("{base}[]");
    let gt_key = format!("{base}[gt]");
    let gte_key = format!("{base}[gte]");
    let lt_key = format!("{base}[lt]");
    let lte_key = format!("{base}[lte]");

    let has_exact = has_any(params, exact_key);
    let has_array = has_any(params, &array_key);
    let has_range = has_any(params, &gt_key)
        || has_any(params, &gte_key)
        || has_any(params, &lt_key)
        || has_any(params, &lte_key);

    let kind_count = (has_exact as u8) + (has_array as u8) + (has_range as u8);
    if kind_count > 1 {
        return Err(bad_request(format!(
            "unsupported parameter combination: do not mix {base}, {base}[], or {base}[gt|gte|lt|lte]"
        )));
    }

    if has_exact {
        let v = get_first(params, exact_key).unwrap_or_default().trim();
        if v.is_empty() {
            return Ok(None);
        }
        return Ok(Some(CostPredicate::Exact(parse_cost_u8(base, v)?)));
    }

    if has_array {
        let Some(values) = parse_cost_array(params, &array_key)? else {
            return Ok(None);
        };
        return Ok(Some(CostPredicate::AnyOf(values)));
    }

    for (key, op) in [
        (gt_key, CompareOp::Gt),
        (gte_key, CompareOp::Gte),
        (lt_key, CompareOp::Lt),
        (lte_key, CompareOp::Lte),
    ] {
        if let Some(v) = get_first(params, &key) {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            return Ok(Some(CostPredicate::Range {
                op,
                value: parse_cost_u8(&key, v)?,
            }));
        }
    }

    Ok(None)
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

fn build_bitmap(state: &AppState, req: &CardsRequest) -> ApiResult<RoaringBitmap> {
    let mut groups = Vec::new();

    // effect[0] searches main lines only (M1/M2/M3) with per-line bucket intersection,
    // then OR across lines. This matches alt-indexer's default per-line query behavior.
    let effect0 = effect0_bitmap_main_lines(
        state,
        &req.filters.effect0_t,
        &req.filters.effect0_c,
        &req.filters.effect0_o,
    );
    if let Some(bmp) = effect0 {
        groups.push(bmp);
    }

    // support[...] searches echo/support line only (Ec), with per-line bucket intersection.
    let support = bitmap_intersect_buckets_on_line(
        state,
        EffectLine::Ec,
        &req.filters.support_t,
        &req.filters.support_c,
        &req.filters.support_o,
    );
    if let Some(bmp) = support {
        groups.push(bmp);
    }

    if !req.factions.is_empty() {
        let mut bmp = RoaringBitmap::new();
        for faction in &req.factions {
            if let Some(f) = state.factions().get(faction) {
                bmp |= f.clone();
            }
        }
        groups.push(bmp);
    }

    if let Some(pred) = &req.main_cost {
        groups.push(bitmap_for_cost_predicate(state, StatField::MainCost, "mainCost", pred)?);
    }
    if let Some(pred) = &req.recall_cost {
        groups.push(bitmap_for_cost_predicate(
            state,
            StatField::RecallCost,
            "recallCost",
            pred,
        )?);
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

fn bitmap_for_cost_predicate(
    state: &AppState,
    field: StatField,
    label: &str,
    pred: &CostPredicate,
) -> ApiResult<RoaringBitmap> {
    let Some(buckets) = state.stats().get(&field) else {
        return Err(bad_request(format!("missing stats index for {label}")));
    };

    let mut out = RoaringBitmap::new();
    match pred {
        CostPredicate::Exact(v) => {
            out |= buckets[*v as usize].clone();
        }
        CostPredicate::AnyOf(values) => {
            for &v in values {
                out |= buckets[v as usize].clone();
            }
        }
        CostPredicate::Range { op, value } => {
            for v in 0u8..=15u8 {
                let keep = match op {
                    CompareOp::Gt => v > *value,
                    CompareOp::Gte => v >= *value,
                    CompareOp::Lt => v < *value,
                    CompareOp::Lte => v <= *value,
                };
                if keep {
                    out |= buckets[v as usize].clone();
                }
            }
        }
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

fn page_cards_v2(
    state: &AppState,
    bitmap: &RoaringBitmap,
    cursor: Option<u32>,
    limit: usize,
) -> ApiResult<(Vec<CardV2>, Option<u32>)> {
    let mut out = Vec::with_capacity(limit);
    let mut last_index: Option<u32> = None;

    let idgd_by_id: BTreeMap<u32, &IdGdCatalogEntry> = state
        .idgd_catalog()
        .entries
        .iter()
        .map(|e| (e.id_gd, e))
        .collect();

    for card_index in bitmap.iter() {
        if cursor.is_some_and(|c| card_index <= c) {
            continue;
        }
        let reference = state
            .decode_reference(card_index)
            .map_err(|e| bad_request(format!("failed to decode reference for card_index {card_index}: {e}")))?;

        let view = state
            .card_view(card_index)
            .ok_or_else(|| bad_request(format!("missing compact record for card_index {card_index}")))?;

        out.push(CardV2 {
            reference,
            main_cost: view.main_cost(),
            recall_cost: view.recall_cost(),
            forest_power: view.forest_power(),
            mountain_power: view.mountain_power(),
            ocean_power: view.ocean_power(),
            faction: CardFaction {
                code: faction_from_code(view.faction_code()),
            },
            main_effect: build_main_effect_localized(&idgd_by_id, &view),
            echo_effect: build_echo_effect_localized(&idgd_by_id, &view),
        });
        last_index = Some(card_index);
        if out.len() >= limit {
            break;
        }
    }

    // Only return a cursor when there may be another page.
    let next_cursor = if out.len() == limit { last_index } else { None };

    Ok((out, next_cursor))
}

fn faction_from_code(code: u8) -> String {
    match code {
        1 => "AX",
        2 => "BR",
        3 => "LY",
        4 => "MU",
        5 => "OR",
        6 => "YZ",
        _ => "UN",
    }
    .to_string()
}

fn build_main_effect_localized(
    idgd_by_id: &BTreeMap<u32, &IdGdCatalogEntry>,
    view: &alt_indexer::compact::CompactCardView<'_>,
) -> BTreeMap<String, String> {
    let groups: [[u16; 3]; 3] = [
        view.main_effect_group(0),
        view.main_effect_group(1),
        view.main_effect_group(2),
    ];

    let mut locales: BTreeMap<String, ()> = BTreeMap::new();
    locales.insert("en_US".to_string(), ());
    for [t, c, o] in groups {
        for id in [t, c, o] {
            if id == 0 {
                continue;
            }
            if let Some(entry) = idgd_by_id.get(&(id as u32)) {
                for k in entry.translations.keys() {
                    locales.insert(k.clone(), ());
                }
            }
        }
    }

    let mut out = BTreeMap::new();
    for locale in locales.keys() {
        let mut lines: Vec<String> = Vec::new();
        for [t, c, o] in groups {
            if let Some(line) = build_effect_line_localized(idgd_by_id, [t, c, o], locale) {
                lines.push(line);
            }
        }
        if !lines.is_empty() {
            out.insert(locale.clone(), lines.join("  "));
        }
    }
    out
}

fn build_echo_effect_localized(
    idgd_by_id: &BTreeMap<u32, &IdGdCatalogEntry>,
    view: &alt_indexer::compact::CompactCardView<'_>,
) -> BTreeMap<String, String> {
    let [t, c, o] = view.echo_effect();

    let mut locales: BTreeMap<String, ()> = BTreeMap::new();
    locales.insert("en_US".to_string(), ());
    for id in [t, c, o] {
        if id == 0 {
            continue;
        }
        if let Some(entry) = idgd_by_id.get(&(id as u32)) {
            for k in entry.translations.keys() {
                locales.insert(k.clone(), ());
            }
        }
    }

    let mut out = BTreeMap::new();
    for locale in locales.keys() {
        if let Some(line) = build_effect_line_localized(idgd_by_id, [t, c, o], locale) {
            out.insert(locale.clone(), line);
        }
    }
    out
}

fn build_effect_line_localized(
    idgd_by_id: &BTreeMap<u32, &IdGdCatalogEntry>,
    [t, c, o]: [u16; 3],
    locale: &str,
) -> Option<String> {
    if t == 0 && c == 0 && o == 0 {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for id in [t, c, o] {
        if id == 0 {
            continue;
        }
        let text = idgd_by_id
            .get(&(id as u32))
            .map(|entry| pick_translation(&entry.translations, locale))
            .unwrap_or_default();
        if !text.is_empty() {
            parts.push(text);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn pick_translation(map: &BTreeMap<String, alt_indexer::card::LocaleText>, locale: &str) -> String {
    if let Some(t) = map.get(locale) {
        return t.text.clone();
    }
    if let Some(t) = map.get("en_US") {
        return t.text.clone();
    }
    map.values().next().map(|t| t.text.clone()).unwrap_or_default()
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
            stats: {
                let mut stats = BTreeMap::<StatField, [RoaringBitmap; 16]>::new();
                let mut main_cost: [RoaringBitmap; 16] =
                    std::array::from_fn(|_| RoaringBitmap::new());
                main_cost[2] = RoaringBitmap::from_iter([2, 5]);
                stats.insert(StatField::MainCost, main_cost);

                let mut recall_cost: [RoaringBitmap; 16] =
                    std::array::from_fn(|_| RoaringBitmap::new());
                recall_cost[3] = RoaringBitmap::from_iter([5]);
                stats.insert(StatField::RecallCost, recall_cost);

                stats
            },
            factions: {
                let mut factions = BTreeMap::<Faction, RoaringBitmap>::new();
                factions.insert(Faction::Ax, RoaringBitmap::from_iter([2, 5]));
                factions.insert(Faction::Br, RoaringBitmap::from_iter([7]));
                factions
            },
        };

        AppState::new(Arc::new(inner))
    }

    #[test]
    fn type_validation_rejects_wrong_kind() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["191".to_string()]); // CONDITION but using [t]
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn effect0_matches_any_main_line() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert!(bmp.contains(2));
        assert!(bmp.contains(5));
        assert_eq!(bmp.len(), 2);
    }

    #[test]
    fn effect0_intersects_across_selectors_on_same_line() {
        let state = test_state();
        // trigger 24 exists on M2 for {2,5}; output 42 exists only on Ec for {5}.
        // Since effect[0] searches M1/M2/M3 and requires same-line intersection, there are no matches.
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("effect[0][o]".to_string(), vec!["42".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert!(bmp.is_empty());
    }

    #[test]
    fn effect_and_support_intersect() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("support[o]".to_string(), vec!["42".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert!(!bmp.contains(2));
        assert!(bmp.contains(5));
        assert_eq!(bmp.len(), 1);
    }

    #[test]
    fn paging_uses_raw_card_index_cursor() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("limit".to_string(), vec!["1".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();

        let (page1, cur1) = page_cards_v2(&state, &bmp, None, 1).unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].reference, "ALT_TEST_B_AX_01_U_3"); // card_index 2 => unique_id 3
        assert_eq!(cur1, Some(2));

        let (page2, cur2) = page_cards_v2(&state, &bmp, cur1, 1).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].reference, "ALT_TEST_B_AX_01_U_6"); // card_index 5 => unique_id 6
        assert_eq!(cur2, Some(5));

        let (page3, cur3) = page_cards_v2(&state, &bmp, cur2, 1).unwrap();
        assert!(page3.is_empty());
        assert_eq!(cur3, None);
    }

    #[test]
    fn parses_factions_from_repeated_or_csv_alias() {
        let state = test_state();

        let mut params: QueryMultiMap = HashMap::new();
        params.insert("faction[]".to_string(), vec!["AX".to_string(), "BR".to_string()]);
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        assert_eq!(req.factions.len(), 2);
        assert!(req.factions.contains(&Faction::Ax));
        assert!(req.factions.contains(&Faction::Br));

        let mut params2: QueryMultiMap = HashMap::new();
        params2.insert("faction".to_string(), vec!["AX,BR".to_string()]);
        params2.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let req2 = parse_request(&state, &params2).unwrap();
        assert_eq!(req2.factions.len(), 2);
        assert!(req2.factions.contains(&Faction::Ax));
        assert!(req2.factions.contains(&Faction::Br));
    }

    #[test]
    fn invalid_faction_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("faction[]".to_string(), vec!["NOPE".to_string()]);
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn cost_exact_array_range_parsing_and_mixing_rejected() {
        let state = test_state();

        // exact
        let mut p1: QueryMultiMap = HashMap::new();
        p1.insert("mainCost".to_string(), vec!["2".to_string()]);
        p1.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r1 = parse_request(&state, &p1).unwrap();
        assert!(matches!(r1.main_cost, Some(CostPredicate::Exact(2))));

        // array (with csv)
        let mut p2: QueryMultiMap = HashMap::new();
        p2.insert("mainCost[]".to_string(), vec!["2,3".to_string()]);
        p2.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r2 = parse_request(&state, &p2).unwrap();
        assert!(matches!(r2.main_cost, Some(CostPredicate::AnyOf(_))));

        // range
        let mut p3: QueryMultiMap = HashMap::new();
        p3.insert("recallCost[lte]".to_string(), vec!["3".to_string()]);
        p3.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r3 = parse_request(&state, &p3).unwrap();
        assert!(matches!(r3.recall_cost, Some(CostPredicate::Range { .. })));

        // reject mixing
        let mut p4: QueryMultiMap = HashMap::new();
        p4.insert("mainCost[]".to_string(), vec!["2".to_string()]);
        p4.insert("mainCost[lte]".to_string(), vec!["3".to_string()]);
        p4.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let err = parse_request(&state, &p4).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn out_of_range_cost_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("mainCost".to_string(), vec!["99".to_string()]);
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn bitmap_intersects_with_faction_and_cost() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        // base ability query matches {2,5}
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        // faction AX is {2,5}
        params.insert("faction[]".to_string(), vec!["AX".to_string()]);
        // recallCost==3 is {5}
        params.insert("recallCost".to_string(), vec!["3".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 1);
        assert!(bmp.contains(5));
    }
}

