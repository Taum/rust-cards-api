use std::collections::{BTreeMap, HashMap};

use url::form_urlencoded;

use alt_indexer::faction_index::Faction;

use crate::http::api::error::{bad_request, ApiResult};
use crate::index::UniquesIndex;
use super::models::{
    AbilityFilters, CardsRequest, CompareOp, CostPredicate, EffectCombineMode,
    EffectSlotFilter, IdGdSelector,
};


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

pub(crate) fn parse_request(state: &UniquesIndex, params: &QueryMultiMap) -> ApiResult<CardsRequest> {
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
        (&gt_key, CompareOp::Gt),
        (&gte_key, CompareOp::Gte),
        (&lt_key, CompareOp::Lt),
        (&lte_key, CompareOp::Lte),
    ] {
        if let Some(v) = get_first(params, key) {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            return Ok(Some(CostPredicate::Range {
                op,
                value: parse_cost_u8(key, v)?,
            }));
        }
    }

    Ok(None)
}

fn validate_idgd_types(state: &UniquesIndex, filters: &AbilityFilters) -> ApiResult<()> {
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
fn parse_u32(field: &str, s: &str) -> ApiResult<u32> {
    s.parse::<u32>()
        .map_err(|_| bad_request(format!("invalid {field} value '{s}'")))
}

fn parse_usize(field: &str, s: &str) -> ApiResult<usize> {
    s.parse::<usize>()
        .map_err(|_| bad_request(format!("invalid {field} value '{s}'")))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::api::cards::test_support::{test_state, test_state_with_sets};
    use crate::index::loader::{SET_CORE, SET_COREKS};
    use crate::http::api::cards::models::CostPredicate;
    use alt_indexer::faction_index::Faction;
    use axum::http::StatusCode;
    use std::collections::HashMap;
    #[test]
    fn parse_with_families_flag() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("withFamilies".to_string(), vec!["".to_string()]);
        let req = parse_request(state.index().as_ref(), &params).unwrap();
        assert!(req.with_families);
    }
    #[test]
    fn type_validation_rejects_wrong_kind() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["191".to_string()]); // CONDITION but using [t]
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
    #[test]
    fn parses_factions_from_repeated_or_csv_alias() {
        let state = test_state();

        let mut params: QueryMultiMap = HashMap::new();
        params.insert("faction[]".to_string(), vec!["AX".to_string(), "BR".to_string()]);
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let req = parse_request(state.index().as_ref(), &params).unwrap();
        assert_eq!(req.factions.len(), 2);
        assert!(req.factions.contains(&Faction::Ax));
        assert!(req.factions.contains(&Faction::Br));

        let mut params2: QueryMultiMap = HashMap::new();
        params2.insert("faction".to_string(), vec!["AX,BR".to_string()]);
        params2.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let req2 = parse_request(state.index().as_ref(), &params2).unwrap();
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
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
    #[test]
    fn parses_sets_from_repeated_or_csv_alias() {
        let state = test_state_with_sets();

        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["CORE".to_string(), "COREKS".to_string()]);
        let req = parse_request(state.index().as_ref(), &params).unwrap();
        assert_eq!(req.sets.len(), 2);
        assert!(req.sets.contains(&SET_CORE.to_string()));
        assert!(req.sets.contains(&SET_COREKS.to_string()));

        let mut params2: QueryMultiMap = HashMap::new();
        params2.insert("set".to_string(), vec!["CORE,COREKS".to_string()]);
        let req2 = parse_request(state.index().as_ref(), &params2).unwrap();
        assert_eq!(req2.sets.len(), 2);
    }

    #[test]
    fn invalid_set_rejected() {
        let state = test_state_with_sets();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("set[]".to_string(), vec!["NOPE".to_string()]);
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
    #[test]
    fn cost_exact_array_range_parsing_and_mixing_rejected() {
        let state = test_state();

        // exact
        let mut p1: QueryMultiMap = HashMap::new();
        p1.insert("mainCost".to_string(), vec!["2".to_string()]);
        p1.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r1 = parse_request(state.index().as_ref(), &p1).unwrap();
        assert!(matches!(r1.main_cost, Some(CostPredicate::Exact(2))));

        // array (with csv)
        let mut p2: QueryMultiMap = HashMap::new();
        p2.insert("mainCost[]".to_string(), vec!["2,3".to_string()]);
        p2.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r2 = parse_request(state.index().as_ref(), &p2).unwrap();
        assert!(matches!(r2.main_cost, Some(CostPredicate::AnyOf(_))));

        // range
        let mut p3: QueryMultiMap = HashMap::new();
        p3.insert("recallCost[lte]".to_string(), vec!["3".to_string()]);
        p3.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let r3 = parse_request(state.index().as_ref(), &p3).unwrap();
        assert!(matches!(r3.recall_cost, Some(CostPredicate::Range { .. })));

        // reject mixing
        let mut p4: QueryMultiMap = HashMap::new();
        p4.insert("mainCost[]".to_string(), vec!["2".to_string()]);
        p4.insert("mainCost[lte]".to_string(), vec!["3".to_string()]);
        p4.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let err = parse_request(state.index().as_ref(), &p4).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn out_of_range_cost_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("mainCost".to_string(), vec!["99".to_string()]);
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
    #[test]
    fn invalid_effect_mode_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][t]".to_string(), vec!["24".to_string()]);
        params.insert("effectMode".to_string(), vec!["xor".to_string()]);
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn invalid_effect_param_key_rejected() {
        let state = test_state();
        let mut params: QueryMultiMap = HashMap::new();
        params.insert("effect[0][x]".to_string(), vec!["24".to_string()]);
        let err = parse_request(state.index().as_ref(), &params).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }
    #[test]
    fn debug_bga_trigram_param_parsing() {
        let state = test_state();
        let mut enabled: QueryMultiMap = HashMap::new();
        enabled.insert("debug_bga_trigram".to_string(), vec![String::new()]);
        let req = parse_request(state.index().as_ref(), &enabled).unwrap();
        assert!(req.debug_bga_trigram);

        let params: QueryMultiMap = HashMap::new();
        let req = parse_request(state.index().as_ref(), &params).unwrap();
        assert!(!req.debug_bga_trigram);
    }
}

