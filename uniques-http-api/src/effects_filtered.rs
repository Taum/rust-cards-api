use std::sync::Arc;

use axum::extract::{RawQuery, State};
use axum::Json;
use roaring::RoaringBitmap;
use serde::Serialize;

use alt_indexer::bitmap::EffectLine;

use crate::cards::{bad_request, build_bitmap, parse_query_multimap, parse_request, ApiResult};
use crate::AppState;

/// `GET /api/v2/effects/filtered` response body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectsFilteredResponse {
    /// Echo of the `editing=<part>:<slot>` request param.
    pub editing: String,
    /// idGds of the edited part that still yield an existing ability under the current filters.
    pub id_gds: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
enum Part {
    Trigger,
    Condition,
    Output,
}

impl Part {
    fn element_type(self) -> &'static str {
        match self {
            Part::Trigger => "TRIGGER",
            Part::Condition => "CONDITION",
            Part::Output => "OUTPUT",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Region {
    /// A main-effect slot (compacted `effect[N]` index). Searches lines M1/M2/M3.
    Main(u32),
    /// The support/echo slot. Searches line Ec.
    Support,
}

const MAIN_LINES: [EffectLine; 3] = [EffectLine::M1, EffectLine::M2, EffectLine::M3];
const SUPPORT_LINES: [EffectLine; 1] = [EffectLine::Ec];

pub async fn get_effects_filtered(
    State(state): State<Arc<AppState>>,
    RawQuery(query): RawQuery,
) -> ApiResult<Json<EffectsFilteredResponse>> {
    let params = parse_query_multimap(query.as_deref())?;

    let editing = params
        .get("editing")
        .and_then(|v| v.first())
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            bad_request(
                "missing required query parameter 'editing' (format '<part>:<slot>', \
                 e.g. trigger:0 or output:support)"
                    .to_string(),
            )
        })?;

    let (part, region) = parse_editing(editing)?;

    // Full current filter state (validates idGd types, including the edited group).
    let req = parse_request(&state, &params)?;

    // Co-constraints = the edited group's other two boxes (the edited part is ignored).
    let (co1, co2) = match region {
        Region::Main(n) => match req.filters.effects.iter().find(|s| s.index == n) {
            Some(slot) => other_two_buckets(part, &slot.t, &slot.c, &slot.o),
            None => (Vec::new(), Vec::new()),
        },
        Region::Support => other_two_buckets(
            part,
            &req.filters.support_t,
            &req.filters.support_c,
            &req.filters.support_o,
        ),
    };

    // Base = the reduced search space with the edited group removed (exact, by index).
    let mut base_req = req.clone();
    match region {
        Region::Main(n) => base_req.filters.effects.retain(|s| s.index != n),
        Region::Support => {
            base_req.filters.support_t.clear();
            base_req.filters.support_c.clear();
            base_req.filters.support_o.clear();
        }
    }
    let base = build_bitmap(&state, &base_req)?;

    let lines: &[EffectLine] = match region {
        Region::Main(_) => &MAIN_LINES,
        Region::Support => &SUPPORT_LINES,
    };

    // Per-line partial: Base intersected with the group's other boxes on that same line.
    let mut partials: Vec<(EffectLine, RoaringBitmap)> = Vec::new();
    for &line in lines {
        let mut pl = base.clone();
        if pl.is_empty() {
            break;
        }
        if !co1.is_empty() {
            pl &= union_on_line(&state, line, &co1);
            if pl.is_empty() {
                continue;
            }
        }
        if !co2.is_empty() {
            pl &= union_on_line(&state, line, &co2);
            if pl.is_empty() {
                continue;
            }
        }
        partials.push((line, pl));
    }

    // A candidate of the edited part's type is reachable iff it shares at least one card with the
    // partial on some line (same-line co-occurrence with the group's other boxes).
    let element_type = part.element_type();
    let mut id_gds: Vec<u32> = Vec::new();
    if !partials.is_empty() {
        for entry in &state.idgd_catalog().entries {
            if entry.element_type != element_type {
                continue;
            }
            let id = entry.id_gd;
            for (line, pl) in &partials {
                if let Some(bm) = state.id_gd_per_line().get(&(id, *line)) {
                    if !pl.is_disjoint(bm) {
                        id_gds.push(id);
                        break;
                    }
                }
            }
        }
    }
    id_gds.sort_unstable();

    Ok(Json(EffectsFilteredResponse {
        editing: editing.to_string(),
        id_gds,
    }))
}

fn parse_editing(editing: &str) -> ApiResult<(Part, Region)> {
    let (part_str, slot_str) = editing.split_once(':').ok_or_else(|| {
        bad_request(format!(
            "invalid editing '{editing}': expected '<part>:<slot>' (e.g. trigger:0)"
        ))
    })?;

    let part = match part_str.trim() {
        "trigger" => Part::Trigger,
        "condition" => Part::Condition,
        "output" => Part::Output,
        other => {
            return Err(bad_request(format!(
                "invalid editing part '{other}': expected 'trigger', 'condition', or 'output'"
            )));
        }
    };

    let region = match slot_str.trim() {
        "support" => Region::Support,
        s => {
            let n = s.parse::<u32>().map_err(|_| {
                bad_request(format!(
                    "invalid editing slot '{s}': expected a main-effect slot index or 'support'"
                ))
            })?;
            Region::Main(n)
        }
    };

    Ok((part, region))
}

/// Returns clones of the two buckets that are *not* the edited part.
fn other_two_buckets(part: Part, t: &[u32], c: &[u32], o: &[u32]) -> (Vec<u32>, Vec<u32>) {
    match part {
        Part::Trigger => (c.to_vec(), o.to_vec()),
        Part::Condition => (t.to_vec(), o.to_vec()),
        Part::Output => (t.to_vec(), c.to_vec()),
    }
}

fn union_on_line(state: &AppState, line: EffectLine, ids: &[u32]) -> RoaringBitmap {
    let mut out = RoaringBitmap::new();
    for &id in ids {
        if let Some(bm) = state.id_gd_per_line().get(&(id, line)) {
            out |= bm;
        }
    }
    out
}
