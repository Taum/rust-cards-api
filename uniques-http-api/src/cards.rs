use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::Json;
use roaring::RoaringBitmap;
use serde::Serialize;
use url::form_urlencoded;

use alt_indexer::bitmap::EffectLine;
use alt_indexer::faction_index::Faction;
use alt_indexer::idgd_catalog::IdGdCatalogEntry;
use alt_indexer::stat_index::StatField;

use crate::loader::{SetBitmaps, SET_CORE, SET_COREKS};
use crate::state::CardResolveError;
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
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSetV2 {
    pub reference: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl From<&alt_indexer::catalog::FamilySet> for CardSetV2 {
    fn from(set: &alt_indexer::catalog::FamilySet) -> Self {
        Self {
            reference: set.reference.clone(),
            name: set.name.clone(),
            code: set.code.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSubTypeV2 {
    pub reference: String,
    pub name: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardV2 {
    pub reference: String,
    pub name: BTreeMap<String, String>,
    pub artist: String,
    pub set: CardSetV2,
    pub card_sub_types: Vec<CardSubTypeV2>,
    pub main_cost: u8,
    pub recall_cost: u8,
    pub forest_power: u8,
    pub mountain_power: u8,
    pub ocean_power: u8,
    pub faction: CardFaction,
    pub main_effect: BTreeMap<String, String>,
    pub echo_effect: BTreeMap<String, String>,
    #[serde(rename = "debug_bga_trigram", skip_serializing_if = "Option::is_none")]
    pub debug_bga_trigram: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyMatchV2 {
    pub family_id: String,
    pub count: u64,
    pub reference: String,
    pub name: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct CardsResponse {
    pub iter: CardsIter,
    pub cards: Vec<CardV2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub families: Option<Vec<FamilyMatchV2>>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

pub(crate) type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EffectCombineMode {
    #[default]
    And,
    Or,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct EffectSlotFilter {
    pub(crate) index: u32,
    pub(crate) t: Vec<u32>,
    pub(crate) c: Vec<u32>,
    pub(crate) o: Vec<u32>,
}

impl EffectSlotFilter {
    fn is_empty(&self) -> bool {
        self.t.is_empty() && self.c.is_empty() && self.o.is_empty()
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AbilityFilters {
    pub(crate) effects: Vec<EffectSlotFilter>,
    effect_mode: EffectCombineMode,
    pub(crate) support_t: Vec<u32>,
    pub(crate) support_c: Vec<u32>,
    pub(crate) support_o: Vec<u32>,
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

#[derive(Debug, Clone)]
pub(crate) struct CardsRequest {
    limit: usize,
    cursor: Option<u32>,
    pub(crate) filters: AbilityFilters,
    factions: Vec<Faction>,
    sets: Vec<String>,
    main_cost: Option<CostPredicate>,
    recall_cost: Option<CostPredicate>,
    name: Option<String>,
    debug_bga_trigram: bool,
    with_families: bool,
}

pub async fn get_card_v2(
    State(state): State<Arc<AppState>>,
    Path(reference): Path<String>,
    RawQuery(query): RawQuery,
) -> ApiResult<Json<CardV2>> {
    let params = parse_query_multimap(query.as_deref())?;
    let debug_bga_trigram = params.contains_key("debug_bga_trigram");

    let card_index = state.resolve_card_index(&reference).map_err(|e| match e {
        CardResolveError::BadRequest { message } => bad_request(message),
        CardResolveError::NotFound { message } => not_found(message),
    })?;

    let view = state.card_view(card_index).ok_or_else(|| {
        not_found(format!("missing compact record for card_index {card_index}"))
    })?;

    if view.faction_code() == 0 {
        return Err(not_found(format!("card not indexed at {reference}")));
    }

    let idgd_by_id: BTreeMap<u32, &IdGdCatalogEntry> = state
        .idgd_catalog()
        .entries
        .iter()
        .map(|e| (e.id_gd, e))
        .collect();

    Ok(Json(card_v2_from_index(
        &state,
        card_index,
        &idgd_by_id,
        debug_bga_trigram,
    )?))
}

pub async fn get_cards_v2(
    State(state): State<Arc<AppState>>,
    RawQuery(query): RawQuery,
) -> ApiResult<Json<CardsResponse>> {
    let params = parse_query_multimap(query.as_deref())?;
    let req = parse_request(&state, &params)?;
    let bitmap = build_bitmap(&state, &req)?;
    let total = bitmap.len() as u64;

    let (cards, next_cursor, families) = if req.with_families && req.cursor.is_none() {
        let (families, example_indices) = families_from_bitmap(&state, &bitmap)?;
        let cards = cards_from_indices(&state, &example_indices, req.debug_bga_trigram)?;
        (cards, None, Some(families))
    } else {
        let (cards, next_cursor) = page_cards_v2(
            &state,
            &bitmap,
            req.cursor,
            req.limit,
            req.debug_bga_trigram,
        )?;
        (cards, next_cursor, None)
    };

    Ok(Json(CardsResponse {
        iter: CardsIter {
            total,
            cursor: next_cursor,
        },
        cards,
        families,
    }))
}

pub(crate) type QueryMultiMap = HashMap<String, Vec<String>>;

pub(crate) fn parse_query_multimap(query: Option<&str>) -> ApiResult<QueryMultiMap> {
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

pub(crate) fn parse_request(state: &AppState, params: &QueryMultiMap) -> ApiResult<CardsRequest> {
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
    filters.effects = parse_effect_slots(params)?;
    filters.effect_mode = parse_effect_mode(params)?;
    filters.support_t = parse_id_list(params, "support[t]")?;
    filters.support_c = parse_id_list(params, "support[c]")?;
    filters.support_o = parse_id_list(params, "support[o]")?;

    let factions = parse_factions(params)?;
    let sets = parse_sets(params)?;
    for code in &sets {
        if !state.set_bitmaps().by_set.contains_key(code) {
            return Err(bad_request(format!("invalid set value '{code}'")));
        }
    }
    let main_cost = parse_cost_predicate(params, "mainCost")?;
    let recall_cost = parse_cost_predicate(params, "recallCost")?;
    let name = parse_name(params);
    let debug_bga_trigram = params.contains_key("debug_bga_trigram");
    let with_families = params.contains_key("withFamilies");

    validate_idgd_types(state, &filters)?;

    Ok(CardsRequest {
        limit,
        cursor,
        filters,
        factions,
        sets,
        main_cost,
        recall_cost,
        name,
        debug_bga_trigram,
        with_families,
    })
}

fn parse_name(params: &QueryMultiMap) -> Option<String> {
    let raw = get_first(params, "name")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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

fn parse_effect_mode(params: &QueryMultiMap) -> ApiResult<EffectCombineMode> {
    let Some(raw) = get_first(params, "effectMode") else {
        return Ok(EffectCombineMode::And);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "and" => Ok(EffectCombineMode::And),
        "or" => Ok(EffectCombineMode::Or),
        other => Err(bad_request(format!(
            "invalid effectMode value '{other}': expected 'and' or 'or'"
        ))),
    }
}

/// Parses `effect[N][t|c|o]` keys into ordered slots (sparse indices allowed).
fn parse_effect_slots(params: &QueryMultiMap) -> ApiResult<Vec<EffectSlotFilter>> {
    let mut by_index: BTreeMap<u32, EffectSlotFilter> = BTreeMap::new();

    for key in params.keys() {
        let Some((index, selector)) = parse_effect_param_key(key)? else {
            continue;
        };
        let ids = parse_id_list(params, key)?;
        let slot = by_index.entry(index).or_insert_with(|| EffectSlotFilter {
            index,
            ..EffectSlotFilter::default()
        });
        match selector {
            IdGdSelector::T => slot.t = ids,
            IdGdSelector::C => slot.c = ids,
            IdGdSelector::O => slot.o = ids,
        }
    }

    Ok(by_index
        .into_values()
        .filter(|slot| !slot.is_empty())
        .collect())
}

/// Returns `None` for keys that are not `effect[N][t|c|o]`.
fn parse_effect_param_key(key: &str) -> ApiResult<Option<(u32, IdGdSelector)>> {
    const PREFIX: &str = "effect[";
    if !key.starts_with(PREFIX) {
        return Ok(None);
    }
    if key == "effectMode" {
        return Ok(None);
    }

    let rest = &key[PREFIX.len()..];
    let bracket_end = rest
        .find(']')
        .ok_or_else(|| bad_request(format!("invalid effect parameter '{key}'")))?;
    let index_str = &rest[..bracket_end];
    if index_str.is_empty() {
        return Err(bad_request(format!("invalid effect parameter '{key}'")));
    }
    let index = parse_u32("effect slot index", index_str)?;

    let field_part = &rest[bracket_end + 1..];
    if !field_part.starts_with('[') || !field_part.ends_with(']') || field_part.len() != 3 {
        return Err(bad_request(format!(
            "invalid effect parameter '{key}': expected effect[N][t], effect[N][c], or effect[N][o]"
        )));
    }
    let selector = match &field_part[1..2] {
        "t" => IdGdSelector::T,
        "c" => IdGdSelector::C,
        "o" => IdGdSelector::O,
        _ => {
            return Err(bad_request(format!(
                "invalid effect parameter '{key}': expected [t], [c], or [o]"
            )));
        }
    };
    Ok(Some((index, selector)))
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

fn parse_sets(params: &QueryMultiMap) -> ApiResult<Vec<String>> {
    // Spec: set[] repeated keys
    // Convenience: set=CORE,COREKS
    let mut codes: Vec<String> = Vec::new();
    if let Some(values) = params.get("set[]") {
        for v in values {
            for part in v.split(',') {
                let s = part.trim();
                if !s.is_empty() {
                    codes.push(s.to_string());
                }
            }
        }
    }
    if let Some(values) = params.get("set") {
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
        if !out.contains(&code) {
            out.push(code);
        }
    }
    Ok(out)
}

fn union_requested_sets(bitmaps: &SetBitmaps, sets: &[String]) -> RoaringBitmap {
    let has_core = sets.iter().any(|s| s == SET_CORE);
    let has_coreks = sets.iter().any(|s| s == SET_COREKS);
    let use_combined = has_core && has_coreks;

    let mut out = RoaringBitmap::new();

    if use_combined {
        if let Some(combined) = &bitmaps.core_and_coreks {
            out = combined.clone();
        } else {
            if let Some(b) = bitmaps.by_set.get(SET_CORE) {
                out |= b.clone();
            }
            if let Some(b) = bitmaps.by_set.get(SET_COREKS) {
                out |= b.clone();
            }
        }
    }

    for code in sets {
        if use_combined && (code == SET_CORE || code == SET_COREKS) {
            continue;
        }
        if let Some(b) = bitmaps.by_set.get(code) {
            out |= b.clone();
        }
    }

    out
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

    for slot in &filters.effects {
        for (key_suffix, selector, ids) in [
            ("[t]", IdGdSelector::T, &slot.t),
            ("[c]", IdGdSelector::C, &slot.c),
            ("[o]", IdGdSelector::O, &slot.o),
        ] {
            let key = format!("effect[{}]{key_suffix}", slot.index);
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
    }

    for (key, selector, ids) in [
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

pub(crate) fn build_bitmap(state: &AppState, req: &CardsRequest) -> ApiResult<RoaringBitmap> {
    let mut groups = Vec::new();

    // Each effect[N] searches main lines only (M1/M2/M3) with per-line bucket intersection,
    // then OR across lines. Multiple slots combine per effectMode (default: and).
    let effect_bitmaps: Vec<RoaringBitmap> = req
        .filters
        .effects
        .iter()
        .filter_map(|slot| {
            effect0_bitmap_main_lines(state, &slot.t, &slot.c, &slot.o)
        })
        .collect();
    if let Some(bmp) = combine_effect_bitmaps(&effect_bitmaps, req.filters.effect_mode) {
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

    if !req.sets.is_empty() {
        groups.push(union_requested_sets(state.set_bitmaps(), &req.sets));
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

    if let Some(name) = &req.name {
        groups.push(state.name_search_index().bitmap_for_contains(state.catalog(), name));
    }

    let mut it = groups.into_iter();
    let mut out = match it.next() {
        Some(first) => first,
        None => all_cards_bitmap(state),
    };
    for bmp in it {
        out &= bmp;
    }
    Ok(out)
}

fn all_cards_bitmap(state: &AppState) -> RoaringBitmap {
    let span = state.manifest().total_bit_span;
    if span == 0 {
        return RoaringBitmap::new();
    }
    let mut bmp = RoaringBitmap::new();
    bmp.insert_range(0..span);
    bmp
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

fn combine_effect_bitmaps(
    bitmaps: &[RoaringBitmap],
    mode: EffectCombineMode,
) -> Option<RoaringBitmap> {
    let mut it = bitmaps.iter();
    let first = it.next()?;
    let mut out = first.clone();
    match mode {
        EffectCombineMode::And => {
            for bmp in it {
                out &= bmp;
            }
        }
        EffectCombineMode::Or => {
            for bmp in it {
                out |= bmp;
            }
        }
    }
    Some(out)
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

fn card_v2_from_index(
    state: &AppState,
    card_index: u32,
    idgd_by_id: &BTreeMap<u32, &IdGdCatalogEntry>,
    debug_bga_trigram: bool,
) -> ApiResult<CardV2> {
    let reference = state
        .decode_reference(card_index)
        .map_err(|e| bad_request(format!("failed to decode reference for card_index {card_index}: {e}")))?;

    let view = state
        .card_view(card_index)
        .ok_or_else(|| bad_request(format!("missing compact record for card_index {card_index}")))?;

    let family = state
        .catalog()
        .family_for_bit(card_index)
        .map_err(|e| bad_request(format!("family lookup for card_index {card_index}: {e}")))?;

    let faction_code = faction_from_code(view.faction_code());
    let faction_name = alt_indexer::faction_display_name(&faction_code)
        .unwrap_or("")
        .to_string();

    Ok(CardV2 {
        reference,
        name: family.name.clone(),
        artist: family.artist.clone(),
        set: CardSetV2::from(&family.set),
        card_sub_types: family
            .card_sub_types
            .iter()
            .map(|st| CardSubTypeV2 {
                reference: st.reference.clone(),
                name: st.name.clone(),
            })
            .collect(),
        main_cost: view.main_cost(),
        recall_cost: view.recall_cost(),
        forest_power: view.forest_power(),
        mountain_power: view.mountain_power(),
        ocean_power: view.ocean_power(),
        faction: CardFaction {
            code: faction_code,
            name: faction_name,
        },
        main_effect: build_main_effect_localized(idgd_by_id, &view),
        echo_effect: build_echo_effect_localized(idgd_by_id, &view),
        debug_bga_trigram: debug_bga_trigram.then(|| build_debug_bga_trigram(&view)),
    })
}

fn first_match_in_range(bitmap: &RoaringBitmap, start: u32, end: u32) -> Option<u32> {
    bitmap.range(start..end).next()
}

fn family_match_from_index(
    state: &AppState,
    card_index: u32,
    family_id: &str,
    count: u64,
) -> ApiResult<FamilyMatchV2> {
    let reference = state
        .decode_reference(card_index)
        .map_err(|e| bad_request(format!("failed to decode reference for card_index {card_index}: {e}")))?;

    let family = state
        .catalog()
        .family_for_bit(card_index)
        .map_err(|e| bad_request(format!("family lookup for card_index {card_index}: {e}")))?;

    Ok(FamilyMatchV2 {
        family_id: family_id.to_string(),
        count,
        reference,
        name: family.name.clone(),
    })
}

fn families_from_bitmap(
    state: &AppState,
    bitmap: &RoaringBitmap,
) -> ApiResult<(Vec<FamilyMatchV2>, Vec<u32>)> {
    let mut families = Vec::new();
    let mut example_indices = Vec::new();
    for group in state.family_span_groups() {
        let count = bitmap.range_cardinality(group.range_start..group.range_end);
        if count == 0 {
            continue;
        }
        let Some(card_index) =
            first_match_in_range(bitmap, group.range_start, group.range_end)
        else {
            continue;
        };
        families.push(family_match_from_index(
            state,
            card_index,
            &group.family_id,
            count,
        )?);
        example_indices.push(card_index);
    }
    Ok((families, example_indices))
}

fn cards_from_indices(
    state: &AppState,
    indices: &[u32],
    debug_bga_trigram: bool,
) -> ApiResult<Vec<CardV2>> {
    let idgd_by_id: BTreeMap<u32, &IdGdCatalogEntry> = state
        .idgd_catalog()
        .entries
        .iter()
        .map(|e| (e.id_gd, e))
        .collect();

    indices
        .iter()
        .map(|&card_index| {
            card_v2_from_index(state, card_index, &idgd_by_id, debug_bga_trigram)
        })
        .collect()
}

fn page_cards_v2(
    state: &AppState,
    bitmap: &RoaringBitmap,
    cursor: Option<u32>,
    limit: usize,
    debug_bga_trigram: bool,
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
        out.push(card_v2_from_index(
            state,
            card_index,
            &idgd_by_id,
            debug_bga_trigram,
        )?);
        last_index = Some(card_index);
        if out.len() >= limit {
            break;
        }
    }

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

fn build_debug_bga_trigram(view: &alt_indexer::compact::CompactCardView<'_>) -> String {
    let mut triplets: Vec<String> = Vec::new();
    for g in 0..3 {
        if let Some(triplet) = format_tco_triplet(view.main_effect_group(g)) {
            triplets.push(triplet);
        }
    }
    if let Some(triplet) = format_tco_triplet(view.echo_effect()) {
        triplets.push(triplet);
    }
    triplets.join(";")
}

fn format_tco_triplet([t, c, o]: [u16; 3]) -> Option<String> {
    if t == 0 && c == 0 && o == 0 {
        return None;
    }
    Some(format!("{t}/{c}/{o}"))
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

pub(crate) fn bad_request(msg: String) -> (StatusCode, Json<ApiError>) {
    (StatusCode::BAD_REQUEST, Json(ApiError { error: msg }))
}

fn not_found(msg: String) -> (StatusCode, Json<ApiError>) {
    (StatusCode::NOT_FOUND, Json(ApiError { error: msg }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use axum::body::Bytes;
    use alt_indexer::catalog::{Catalog, FamilyCardSubType, FamilyEntry, FamilySet, FACTION_ORDER};
    use alt_indexer::compact::{encode_record, CompactCardFields, RECORD_SIZE};
    use alt_indexer::faction_index::Faction;
    use alt_indexer::idgd_catalog::{IdGdCatalog, IdGdCatalogEntry};
    use alt_indexer::stat_index::StatField;

    use crate::loader::{
        build_family_lookup_index, build_family_span_groups, build_name_search_index,
        build_set_bitmaps, FactionsSummary,
        IndexManifest, StatsSummary, SET_CORE, SET_COREKS,
    };
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
                name: BTreeMap::from([
                    ("en_US".to_string(), "Test Card".to_string()),
                    ("fr_FR".to_string(), "Élémentaire de Kélon".to_string()),
                ]),
                artist: "Test Artist".to_string(),
                card_sub_types: vec![FamilyCardSubType {
                    reference: "ENGINEER".to_string(),
                    name: BTreeMap::from([("en_US".to_string(), "Engineer".to_string())]),
                }],
                set: FamilySet {
                    reference: "COREKS".to_string(),
                    name: "Test Set".to_string(),
                    code: Some("BTG".to_string()),
                },
            }],
            total_bit_span: 10,
        };

        let manifest = IndexManifest {
            version: 1,
            set: "TEST".to_string(),
            kind: None,
            built_at_secs: 0,
            card_count: 10,
            id_gd_count: 4,
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
                    is_main: true,
                    is_echo: false,
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
                    is_main: true,
                    is_echo: false,
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
                    is_main: false,
                    is_echo: true,
                },
                IdGdCatalogEntry {
                    id_gd: 90,
                    card_count: 1,
                    bitmap_bytes: 0,
                    bitmap_file: "90.roar".to_string(),
                    element_type: "OUTPUT".to_string(),
                    translations: BTreeMap::new(),
                    m1: None,
                    m2: None,
                    m3: None,
                    ec: None,
                    is_main: true,
                    is_echo: false,
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
        id_gd_per_line.insert((90, EffectLine::M1), RoaringBitmap::from_iter([2]));

        let mut cards = vec![0u8; 10 * RECORD_SIZE];
        let ax_record = encode_record(&CompactCardFields {
            faction_code: 1,
            main_cost: 2,
            recall_cost: 3,
            mountain_power: 0,
            ocean_power: 0,
            forest_power: 0,
            main_effect: [[24, 191, 90], [0, 0, 0], [0, 0, 0]],
            echo_effect: [24, 0, 42],
        });
        for &idx in &[2u32, 5] {
            let off = idx as usize * RECORD_SIZE;
            cards[off..off + RECORD_SIZE].copy_from_slice(&ax_record);
        }

        let effects_list = crate::effects::build_effects_list(&idgd_catalog);
        let effects_body = Arc::new(crate::effects::serialize_effects_list(&effects_list).unwrap());
        let set_bitmaps = build_set_bitmaps(&catalog);
        let name_search_index = build_name_search_index(&catalog);
        let family_lookup_index = build_family_lookup_index(&catalog);
        let family_span_groups = build_family_span_groups(&catalog);

        let inner = AppStateInner {
            index_dir: "C:\\tmp\\index".into(),
            catalog,
            manifest,
            idgd_catalog,
            effects_body,
            stats_summary,
            factions_summary,
            cards,
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
            set_bitmaps,
            name_search_index,
            family_lookup_index,
            family_span_groups,
        };

        AppState::new(Arc::new(inner))
    }

    fn family_entry(
        start_bit: u32,
        source_set: &str,
        family_number: &str,
        name: &str,
    ) -> FamilyEntry {
        FamilyEntry {
            start_bit,
            faction: "AX".to_string(),
            family_number: family_number.to_string(),
            family_id: format!("AX_{family_number}"),
            source_set: Some(source_set.to_string()),
            max_unique_id: 5,
            card_count: 5,
            first_reference: format!("ALT_{source_set}_B_AX_{family_number}_U_1"),
            name: BTreeMap::from([("en_US".to_string(), name.to_string())]),
            artist: "Test".to_string(),
            card_sub_types: vec![],
            set: FamilySet {
                reference: source_set.to_string(),
                name: source_set.to_string(),
                code: None,
            },
        }
    }

    fn test_state_with_sets() -> AppState {
        let catalog = Catalog {
            set: "ALL_SETS".to_string(),
            faction_order: FACTION_ORDER.iter().map(|s| s.to_string()).collect(),
            families: vec![
                family_entry(0, SET_CORE, "01", "Kelon Elemental"),
                family_entry(5, SET_COREKS, "01", "Kelon Elemental"),
                family_entry(10, "ALIZE", "02", "Other Character"),
            ],
            total_bit_span: 15,
        };

        let manifest = IndexManifest {
            version: 1,
            set: "ALL_SETS".to_string(),
            kind: Some("merge".to_string()),
            built_at_secs: 0,
            card_count: 15,
            id_gd_count: 0,
            total_bit_span: 15,
            family_count: 3,
        };

        let set_bitmaps = build_set_bitmaps(&catalog);
        let name_search_index = build_name_search_index(&catalog);
        let family_lookup_index = build_family_lookup_index(&catalog);
        let family_span_groups = build_family_span_groups(&catalog);
        assert!(set_bitmaps.by_set.contains_key(SET_CORE));
        assert!(set_bitmaps.by_set.contains_key(SET_COREKS));
        assert!(set_bitmaps.by_set.contains_key("ALIZE"));
        assert!(set_bitmaps.core_and_coreks.is_some());

        let inner = AppStateInner {
            index_dir: "C:\\tmp\\index".into(),
            catalog,
            manifest,
            idgd_catalog: IdGdCatalog {
                set: "ALL_SETS".to_string(),
                entries: vec![],
            },
            effects_body: Arc::new(Bytes::from_static(b"{}")),
            stats_summary: StatsSummary {
                version: 1,
                set: "ALL_SETS".to_string(),
                total_cards_indexed: 15,
                fields: vec![],
            },
            factions_summary: FactionsSummary {
                version: 1,
                set: "ALL_SETS".to_string(),
                total_cards_indexed: 15,
                source: "test".to_string(),
                factions: vec![],
                unknown_count: 0,
                bitmap_dir: "factions".to_string(),
            },
            cards: vec![0u8; 15 * RECORD_SIZE],
            id_gd_whole: BTreeMap::new(),
            id_gd_per_line: BTreeMap::new(),
            stats: BTreeMap::new(),
            factions: BTreeMap::new(),
            set_bitmaps,
            name_search_index,
            family_lookup_index,
            family_span_groups,
        };

        AppState::new(Arc::new(inner))
    }

    #[test]
    fn parse_with_families_flag() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("withFamilies".to_string(), vec!["".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        assert!(req.with_families);
    }

    #[test]
    fn families_from_bitmap_merges_core_coreks_span() {
        let state = test_state_with_sets();
        let groups = state.family_span_groups();
        assert_eq!(groups.len(), 2);
        let ax01 = groups.iter().find(|g| g.family_id == "AX_01").unwrap();
        assert_eq!(ax01.range_start, 0);
        assert_eq!(ax01.range_end, 10);

        let mut bmp = RoaringBitmap::new();
        bmp.insert(1);
        bmp.insert(6);

        let (families, ensure) = families_from_bitmap(&state, &bmp).unwrap();
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].family_id, "AX_01");
        assert_eq!(families[0].count, 2);
        assert_eq!(families[0].reference, "ALT_CORE_B_AX_01_U_2");
        assert_eq!(
            families[0].name.get("en_US").map(String::as_str),
            Some("Kelon Elemental")
        );

        let cards = cards_from_indices(&state, &ensure, false).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].reference, families[0].reference);
    }

    #[test]
    fn cards_from_indices_matches_families_only() {
        let state = test_state_with_sets();
        let mut bmp = RoaringBitmap::new();
        for i in 0..15 {
            bmp.insert(i);
        }
        let (families, ensure) = families_from_bitmap(&state, &bmp).unwrap();
        assert_eq!(families.len(), 2);

        let cards = cards_from_indices(&state, &ensure, false).unwrap();
        assert_eq!(cards.len(), families.len());
        for (card, fam) in cards.iter().zip(&families) {
            assert_eq!(card.reference, fam.reference);
        }
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
    fn card_v2_includes_family_metadata() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("limit".to_string(), vec!["1".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        let (page, _) = page_cards_v2(&state, &bmp, None, 1, false).unwrap();
        let card = &page[0];
        assert_eq!(card.name.get("en_US").map(String::as_str), Some("Test Card"));
        assert_eq!(card.artist, "Test Artist");
        assert_eq!(card.set.code.as_deref(), Some("BTG"));
        assert_eq!(card.faction.code, "AX");
        assert_eq!(card.faction.name, "Axiom");
        assert_eq!(card.card_sub_types[0].reference, "ENGINEER");
    }

    #[test]
    fn paging_uses_raw_card_index_cursor() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("limit".to_string(), vec!["1".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();

        let (page1, cur1) = page_cards_v2(&state, &bmp, None, 1, false).unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].reference, "ALT_TEST_B_AX_01_U_3"); // card_index 2 => unique_id 3
        assert_eq!(cur1, Some(2));

        let (page2, cur2) = page_cards_v2(&state, &bmp, cur1, 1, false).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].reference, "ALT_TEST_B_AX_01_U_6"); // card_index 5 => unique_id 6
        assert_eq!(cur2, Some(5));

        let (page3, cur3) = page_cards_v2(&state, &bmp, cur2, 1, false).unwrap();
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
    fn parses_sets_from_repeated_or_csv_alias() {
        let state = test_state_with_sets();

        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["CORE".to_string(), "COREKS".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        assert_eq!(req.sets.len(), 2);
        assert!(req.sets.contains(&SET_CORE.to_string()));
        assert!(req.sets.contains(&SET_COREKS.to_string()));

        let mut params2: QueryMultiMap = HashMap::new();
        params2.insert("set".to_string(), vec!["CORE,COREKS".to_string()]);
        let req2 = parse_request(&state, &params2).unwrap();
        assert_eq!(req2.sets.len(), 2);
    }

    #[test]
    fn invalid_set_rejected() {
        let state = test_state_with_sets();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["NOPE".to_string()]);
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn set_filter_uses_combined_core_coreks_bitmap() {
        let state = test_state_with_sets();
        let bitmaps = state.set_bitmaps();

        let manual = bitmaps.by_set[SET_CORE].clone() | bitmaps.by_set[SET_COREKS].clone();
        let combined = bitmaps.core_and_coreks.as_ref().unwrap();
        assert_eq!(manual, *combined);

        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["CORE".to_string(), "COREKS".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp, *combined);
        assert_eq!(bmp.len(), 10);

        let mut single: QueryMultiMap = HashMap::new();
        single.insert("set[]".to_string(), vec!["CORE".to_string()]);
        let req_single = parse_request(&state, &single).unwrap();
        let bmp_single = build_bitmap(&state, &req_single).unwrap();
        assert_eq!(bmp_single.len(), 5);
        assert!(bmp_single.contains(0));
        assert!(!bmp_single.contains(5));

        let mut three: QueryMultiMap = HashMap::new();
        three.insert(
            "set[]".to_string(),
            vec!["CORE".to_string(), "COREKS".to_string(), "ALIZE".to_string()],
        );
        let req_three = parse_request(&state, &three).unwrap();
        let bmp_three = build_bitmap(&state, &req_three).unwrap();
        assert_eq!(bmp_three.len(), 15);
    }

    #[test]
    fn set_only_query_without_other_filters() {
        let state = test_state_with_sets();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["ALIZE".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 5);
        for i in 10..15 {
            assert!(bmp.contains(i));
        }
    }

    #[test]
    fn empty_name_treated_as_no_filter() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["   ".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        assert!(req.name.is_none());
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), state.manifest().total_bit_span as u64);
    }

    #[test]
    fn name_contains_case_insensitive() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["test card".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 10);
    }

    #[test]
    fn name_matches_any_locale() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["kelon".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 10);
    }

    #[test]
    fn name_folds_unicode_diacritics() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["elementaire".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(
            bmp.len(),
            10,
            "elementaire should match fr_FR Élémentaire de Kélon"
        );
    }

    #[test]
    fn name_no_match_returns_empty_bitmap() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["zzzzz".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert!(bmp.is_empty());
    }

    #[test]
    fn name_only_query_without_other_filters() {
        let state = test_state_with_sets();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["Kelon".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 10);
        for i in 0..10 {
            assert!(bmp.contains(i));
        }
        assert!(!bmp.contains(10));
    }

    #[test]
    fn name_combined_with_set_filter() {
        let state = test_state_with_sets();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("name".to_string(), vec!["Kelon".to_string()]);
        params.insert("set[]".to_string(), vec!["CORE".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), 5);
        for i in 0..5 {
            assert!(bmp.contains(i));
        }
        assert!(!bmp.contains(5));
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
    fn no_filters_returns_all_cards() {
        let state = test_state();
        let params: QueryMultiMap = HashMap::new();
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        assert_eq!(bmp.len(), state.manifest().total_bit_span as u64);
        for i in 0..state.manifest().total_bit_span {
            assert!(bmp.contains(i));
        }
    }

    #[test]
    fn multiple_effect_slots_combine_with_effect_mode() {
        let state = test_state();
        // slot 0: trigger 24 on M2 -> {2,5}; slot 1: output 90 on M1 -> {2}
        let mut and_params: QueryMultiMap = HashMap::new();
        and_params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        and_params.insert("effect[1][o]".to_string(), vec!["90".to_string()]);
        and_params.insert("effectMode".to_string(), vec!["and".to_string()]);
        let and_req = parse_request(&state, &and_params).unwrap();
        let and_bmp = build_bitmap(&state, &and_req).unwrap();
        assert!(and_bmp.contains(2));
        assert!(!and_bmp.contains(5));
        assert_eq!(and_bmp.len(), 1);

        let mut or_params: QueryMultiMap = HashMap::new();
        or_params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        or_params.insert("effect[1][o]".to_string(), vec!["90".to_string()]);
        or_params.insert("effectMode".to_string(), vec!["or".to_string()]);
        let or_req = parse_request(&state, &or_params).unwrap();
        let or_bmp = build_bitmap(&state, &or_req).unwrap();
        assert!(or_bmp.contains(2));
        assert!(or_bmp.contains(5));
        assert_eq!(or_bmp.len(), 2);

        // default effectMode is and
        let mut default_params: QueryMultiMap = HashMap::new();
        default_params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        default_params.insert("effect[1][o]".to_string(), vec!["90".to_string()]);
        let default_req = parse_request(&state, &default_params).unwrap();
        assert_eq!(default_req.filters.effect_mode, EffectCombineMode::And);
        let default_bmp = build_bitmap(&state, &default_req).unwrap();
        assert_eq!(default_bmp.len(), 1);
        assert!(default_bmp.contains(2));
    }

    #[test]
    fn invalid_effect_mode_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("effectMode".to_string(), vec!["xor".to_string()]);
        let err = parse_request(&state, &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn invalid_effect_param_key_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][x]".to_string(), vec!["24".to_string()]);
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

    #[test]
    fn debug_bga_trigram_param_parsing() {
        let state = test_state();
        let mut enabled: QueryMultiMap = HashMap::new();
        enabled.insert("debug_bga_trigram".to_string(), vec![String::new()]);
        let req = parse_request(&state, &enabled).unwrap();
        assert!(req.debug_bga_trigram);

        let params: QueryMultiMap = HashMap::new();
        let req = parse_request(&state, &params).unwrap();
        assert!(!req.debug_bga_trigram);
    }

    #[test]
    fn debug_bga_trigram_includes_main_and_echo_tcos() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("limit".to_string(), vec!["1".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        let (page, _) = page_cards_v2(&state, &bmp, None, 1, true).unwrap();
        assert_eq!(page[0].debug_bga_trigram.as_deref(), Some("24/191/90;24/0/42"));

        let (page, _) = page_cards_v2(&state, &bmp, None, 1, false).unwrap();
        assert!(page[0].debug_bga_trigram.is_none());
    }

    #[test]
    fn debug_bga_trigram_serializes_on_card() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("limit".to_string(), vec!["1".to_string()]);
        let req = parse_request(&state, &params).unwrap();
        let bmp = build_bitmap(&state, &req).unwrap();
        let (page, _) = page_cards_v2(&state, &bmp, None, 1, true).unwrap();
        let json = serde_json::to_value(&page[0]).unwrap();
        assert_eq!(json["debug_bga_trigram"], "24/191/90;24/0/42");
    }
}

