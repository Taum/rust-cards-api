use crate::combo::{id_gd_set, id_gd_set_id, shape_label, shape_of, strict_tuple, strict_tuple_id};
use crate::locale::{
    apply_locale_fractions, card_json_path, load_locale_availability, LocaleAvailability,
    LocalePolicy,
};
use crate::plan::{CardIdentity, PlanCard};
use crate::progress::{self, StepGuard};
use anyhow::{bail, Context, Result};
use indicatif::ProgressBar;
use index_core::catalog::Catalog;
use index_core::compact::{CompactCardView, RECORD_SIZE};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum ComboMode {
    Set,
    Tuple,
    Shape,
}

impl ComboMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "set" => Ok(ComboMode::Set),
            "tuple" => Ok(ComboMode::Tuple),
            "shape" => Ok(ComboMode::Shape),
            other => bail!("unknown combo-mode '{other}' (use set, tuple, or shape)"),
        }
    }
}

pub struct SampleOptions {
    pub index_dir: PathBuf,
    pub set: String,
    pub budget: usize,
    pub locale_policy: LocalePolicy,
    pub combo_mode: ComboMode,
    pub seed: u64,
    pub equinox_root: PathBuf,
    pub out_plan: PathBuf,
    pub out_summary: PathBuf,
}

#[derive(Debug, Clone)]
struct FamilyMeta {
    family_id: String,
    set: String,
    faction: String,
    family_number: String,
}

#[derive(Debug, Clone, Copy)]
struct CompactCandidate {
    family_idx: u32,
    unique_id: u32,
    shape_mask: u8,
    tuple_idx: u32,
    combo_idx: u32,
}

#[derive(Debug, Clone)]
struct ShapeSlot {
    family_id: String,
    shape: u8,
    tuple_indices: Vec<u32>,
    candidate_indices: Vec<usize>,
    by_tuple: HashMap<u32, Vec<usize>>,
}

struct CandidateStore {
    families: Vec<FamilyMeta>,
    combos: Vec<String>,
    candidates: Vec<CompactCandidate>,
    shape_slots: Vec<ShapeSlot>,
    /// `family_id` values that exist in both CORE and COREKS in the merged index.
    core_ks_shared: HashSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UncoveredShapeSlot {
    pub family_id: String,
    pub shape: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SampleSummary {
    pub index_dir: String,
    pub set: String,
    pub seed: u64,
    pub combo_mode: String,
    pub budget: usize,
    pub shape_coverage_minimum: usize,
    pub locale_policy: LocalePolicySummary,
    pub by_tier: BTreeMap<String, usize>,
    pub image_slots_planned: usize,
    pub total_valid_cards: u64,
    pub distinct_family_ids: usize,
    pub shape_slots_required: usize,
    pub shape_slots_achievable: usize,
    pub shape_slots_covered: usize,
    pub shape_slots_uncovered: Vec<UncoveredShapeSlot>,
    pub shape_slots_unpicked: Vec<UncoveredShapeSlot>,
    pub cards_skipped_no_en: usize,
    pub distinct_tuples_picked: usize,
    pub picks_phase1_shape_coverage: usize,
    pub picks_phase2_family_quota: usize,
    pub cards_picked: usize,
    pub plan_cards: usize,
    pub by_set: BTreeMap<String, usize>,
    pub by_family_top20: Vec<FamilyPickCount>,
    pub by_family_distribution: FamilyDistribution,
    pub by_locale: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalePolicySummary {
    pub full_fraction: f64,
    pub fr_fraction: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilyPickCount {
    pub family_id: String,
    pub set: String,
    pub picks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilyDistribution {
    pub families_total: usize,
    pub families_with_picks: usize,
    pub min_picks: usize,
    pub max_picks: usize,
    pub mean_picks: f64,
}

pub fn shape_slots_required(index_dir: &Path, set: &str) -> Result<usize> {
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_data = std::fs::read(set_dir.join("cards.bin"))?;
    Ok(count_shape_slots(&catalog, &cards_data))
}

pub fn shape_coverage_minimum(index_dir: &Path, set: &str) -> Result<usize> {
    let (slots, _) = build_shape_slots_from_index(index_dir, set)?;
    Ok(slots.len())
}

pub fn run(opts: &SampleOptions) -> Result<SampleSummary> {
    const P: &str = "sample";
    progress::log(P, &format!("budget={}, set={}", opts.budget, opts.set));

    let set_dir = opts.index_dir.join(&opts.set);
    let (catalog, cards_data) = {
        let step = StepGuard::begin(P, "load index");
        let catalog = Catalog::load(&set_dir.join("catalog.json"))
            .with_context(|| format!("load catalog from {}", set_dir.display()))?;
        let cards_data = std::fs::read(set_dir.join("cards.bin"))
            .with_context(|| format!("read cards.bin from {}", set_dir.display()))?;
        step.finish(Some(&format!(
            "catalog: {} families, cards.bin: {} bytes",
            catalog.families.len(),
            cards_data.len()
        )));
        (catalog, cards_data)
    };

    anyhow::ensure!(opts.budget > 0, "budget must be > 0");
    opts.locale_policy.validate()?;

    let store = {
        let expected_bits = catalog.total_bit_span as u64;
        let scan_pb = progress::bar(P, "scan cards.bin", expected_bits);
        let store = build_candidate_store(&catalog, &cards_data, opts.combo_mode, Some(&scan_pb))?;
        progress::finish_bar(
            scan_pb,
            format!(
                "{} valid candidates ({} CORE∩COREKS families)",
                store.candidates.len(),
                store.core_ks_shared.len()
            ),
        );
        store
    };

    let (shape_slots_required, tuple_plan) = {
        let step = StepGuard::begin(P, "assign shape tuples");
        let shape_slots_required = store.shape_slots.len();
        let tuple_plan = assign_shape_tuples_from_slots(&store.shape_slots, &store.families);
        step.finish(Some(&format!(
            "{}/{} slots with unique strict tuples ({} need duplicate-ok pick)",
            tuple_plan.assigned.len(),
            shape_slots_required,
            tuple_plan.tuple_collisions.len()
        )));
        (shape_slots_required, tuple_plan)
    };
    anyhow::ensure!(
        opts.budget >= shape_slots_required,
        "budget {} is below shape-coverage minimum {} \
         ({} distinct family+shape slots)",
        opts.budget,
        shape_slots_required,
        shape_slots_required,
    );
    let total_valid = store.candidates.len() as u64;
    anyhow::ensure!(total_valid > 0, "no valid cards found in index");

    let mut rng = ChaCha8Rng::seed_from_u64(opts.seed);
    let mut picked = vec![false; store.candidates.len()];
    let mut used_tuples: HashSet<u32> = HashSet::new();
    let mut family_picks: HashMap<String, usize> = HashMap::new();
    let mut set_picks: HashMap<(String, String), usize> = HashMap::new();
    let mut shape_pick_counts: HashMap<(String, u8), usize> = HashMap::new();
    let mut chosen_order: Vec<usize> = Vec::new();

    let mut picks_phase1 = 0usize;
    let shape_slots_achievable = tuple_plan.assigned.len();
    let shape_slots_uncovered: Vec<UncoveredShapeSlot> = tuple_plan
        .tuple_collisions
        .iter()
        .map(|slot| UncoveredShapeSlot {
            family_id: slot.family_id.clone(),
            shape: shape_label(slot.shape),
        })
        .collect();
    let mut shape_slots_unpicked: Vec<UncoveredShapeSlot> = Vec::new();

    let phase1_len = (tuple_plan.assigned.len() + tuple_plan.tuple_collisions.len())
        .min(opts.budget) as u64;
    let phase1_pb = progress::bar(P, "phase 1 shape floor", phase1_len.max(1));

    for (slot, preferred_tuple_idx) in &tuple_plan.assigned {
        if chosen_order.len() >= opts.budget {
            break;
        }
        let Some(idx) = pick_shape_floor_card(
            &store,
            slot,
            *preferred_tuple_idx,
            &used_tuples,
            &set_picks,
            &mut rng,
        ) else {
            shape_slots_unpicked.push(UncoveredShapeSlot {
                family_id: slot.family_id.clone(),
                shape: shape_label(slot.shape),
            });
            phase1_pb.inc(1);
            continue;
        };
        record_pick(
            idx,
            &store,
            &mut picked,
            &mut used_tuples,
            &mut family_picks,
            &mut set_picks,
            &mut shape_pick_counts,
            &mut chosen_order,
        );
        picks_phase1 += 1;
        phase1_pb.inc(1);
    }

    for slot in &tuple_plan.tuple_collisions {
        if chosen_order.len() >= opts.budget {
            break;
        }
        let Some(idx) =
            pick_shape_floor_card_allow_duplicate(&store, slot, &picked, &set_picks, &mut rng)
        else {
            shape_slots_unpicked.push(UncoveredShapeSlot {
                family_id: slot.family_id.clone(),
                shape: shape_label(slot.shape),
            });
            phase1_pb.inc(1);
            continue;
        };
        record_pick(
            idx,
            &store,
            &mut picked,
            &mut used_tuples,
            &mut family_picks,
            &mut set_picks,
            &mut shape_pick_counts,
            &mut chosen_order,
        );
        picks_phase1 += 1;
        phase1_pb.inc(1);
    }
    progress::finish_bar(phase1_pb, format!("{picks_phase1} shape-floor picks"));

    let shape_slots_covered = picks_phase1;

    let bucket_pb = progress::bar(P, "build family buckets", store.candidates.len() as u64);
    let mut by_family_remaining: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in store.candidates.iter().enumerate() {
        if !picked[i] && !used_tuples.contains(&c.tuple_idx) {
            let family_id = store.families[c.family_idx as usize].family_id.clone();
            by_family_remaining.entry(family_id).or_default().push(i);
        }
        if (i + 1) % 50_000 == 0 || i + 1 == store.candidates.len() {
            bucket_pb.set_position((i + 1) as u64);
        }
    }
    progress::finish_bar(
        bucket_pb,
        format!("{} families with remaining candidates", by_family_remaining.len()),
    );

    let mut picks_phase2 = 0usize;
    let mut family_indices: Vec<String> = by_family_remaining.keys().cloned().collect();
    family_indices.sort_unstable();
    let phase2_target = opts.budget.saturating_sub(chosen_order.len()) as u64;
    let phase2_pb = progress::bar(P, "phase 2 family quota", phase2_target.max(1));
    'outer: for _ in 0..opts.budget {
        if chosen_order.len() >= opts.budget {
            break;
        }
        let mut min_picks = usize::MAX;
        for f in &family_indices {
            if by_family_remaining
                .get(f)
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            {
                let p = family_picks.get(f).copied().unwrap_or(0);
                if p < min_picks {
                    min_picks = p;
                }
            }
        }
        if min_picks == usize::MAX {
            break;
        }
        let mut progressed = false;
        for f in family_indices.clone() {
            if chosen_order.len() >= opts.budget {
                break 'outer;
            }
            let Some(bucket) = by_family_remaining.get_mut(&f) else {
                continue;
            };
            if bucket.is_empty() {
                continue;
            }
            if family_picks.get(&f).copied().unwrap_or(0) != min_picks {
                continue;
            }
            while family_picks.get(&f).copied().unwrap_or(0) == min_picks {
                if bucket.is_empty() {
                    break;
                }
                let best_pos = best_bucket_pick(&store, bucket, &shape_pick_counts, &f, &set_picks);
                let idx = bucket.swap_remove(best_pos);
                if used_tuples.contains(&store.candidates[idx].tuple_idx) {
                    continue;
                }
                record_pick(
                    idx,
                    &store,
                    &mut picked,
                    &mut used_tuples,
                    &mut family_picks,
                    &mut set_picks,
                    &mut shape_pick_counts,
                    &mut chosen_order,
                );
                picks_phase2 += 1;
                phase2_pb.inc(1);
                progressed = true;
                break;
            }
        }
        if !progressed {
            break;
        }
    }
    progress::finish_bar(
        phase2_pb,
        format!("{picks_phase2} quota picks ({} total)", chosen_order.len()),
    );

    let mut avail_cache: HashMap<String, LocaleAvailability> = HashMap::new();
    let mut picked_indices: Vec<usize> = Vec::new();
    let mut picked_avail: Vec<LocaleAvailability> = Vec::new();
    let mut cards_skipped_no_en = 0usize;
    let locale_pb = progress::bar(P, "read equinox locales", chosen_order.len() as u64);
    for (i, &idx) in chosen_order.iter().enumerate() {
        let avail = availability_for(&store, idx, &opts.equinox_root, &mut avail_cache);
        if !avail.en {
            cards_skipped_no_en += 1;
        } else {
            picked_indices.push(idx);
            picked_avail.push(avail);
        }
        locale_pb.set_position((i + 1) as u64);
    }
    progress::finish_bar(
        locale_pb,
        format!(
            "{} cards with en_US ({} skipped)",
            picked_indices.len(),
            cards_skipped_no_en
        ),
    );

    let planned_locales = {
        let step = StepGuard::begin(P, "apply locale fractions");
        let planned =
            apply_locale_fractions(&picked_avail, &opts.locale_policy, &mut rng);
        step.finish(Some(&format!(
            "full~{:.1}%, fr~{:.1}% targets on {} cards",
            opts.locale_policy.full_fraction * 100.0,
            opts.locale_policy.fr_fraction * 100.0,
            planned.len()
        )));
        planned
    };

    if let Some(parent) = opts.out_plan.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    let mut plan_writer = BufWriter::new(
        File::create(&opts.out_plan)
            .with_context(|| format!("create {}", opts.out_plan.display()))?,
    );
    let mut by_locale: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_tier: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_set: BTreeMap<String, usize> = BTreeMap::new();
    let mut plan_cards = 0usize;
    let mut image_slots_planned = 0usize;
    let write_pb = progress::bar(P, "write plan.jsonl", picked_indices.len() as u64);
    for (&idx, plan) in picked_indices.iter().zip(planned_locales.iter()) {
        let shape_floor = chosen_order
            .iter()
            .position(|&c| c == idx)
            .is_some_and(|pos| pos < picks_phase1);
        let card = card_identity(&store, idx);
        *by_set.entry(card.set.clone()).or_insert(0) += 1;
        *by_tier.entry(plan.tier.as_str().to_string()).or_insert(0) += 1;
        for loc in &plan.locales {
            *by_locale.entry(loc.clone()).or_insert(0) += 1;
            image_slots_planned += 1;
        }
        let row = PlanCard {
            card,
            locale_tier: plan.tier,
            locales: plan.locales.clone(),
            shape_floor,
        };
        serde_json::to_writer(&mut plan_writer, &row)?;
        plan_writer.write_all(b"\n")?;
        plan_cards += 1;
        write_pb.inc(1);
    }
    plan_writer.flush()?;
    progress::finish_bar(
        write_pb,
        format!("{} rows → {}", plan_cards, opts.out_plan.display()),
    );

    let family_picks_by_id: BTreeMap<String, usize> = family_picks.into_iter().collect();
    let summary = build_summary(
        opts,
        shape_slots_required,
        total_valid,
        family_picks_by_id.len(),
        shape_slots_required,
        shape_slots_achievable,
        shape_slots_covered,
        shape_slots_uncovered,
        shape_slots_unpicked,
        cards_skipped_no_en,
        used_tuples.len(),
        picks_phase1,
        picks_phase2,
        chosen_order.len(),
        plan_cards,
        image_slots_planned,
        by_set,
        family_picks_by_id,
        by_locale,
        by_tier,
    );
    {
        let step = StepGuard::begin(P, "write summary");
        write_summary(&opts.out_summary, &summary)?;
        step.finish(Some(&opts.out_summary.display().to_string()));
    }
    Ok(summary)
}

fn build_candidate_store(
    catalog: &Catalog,
    cards_data: &[u8],
    mode: ComboMode,
    progress: Option<&ProgressBar>,
) -> Result<CandidateStore> {
    let expected_len = catalog.total_bit_span as usize * RECORD_SIZE;
    anyhow::ensure!(
        cards_data.len() >= expected_len,
        "cards.bin size {} smaller than expected {}",
        cards_data.len(),
        expected_len
    );

    let mut families: Vec<FamilyMeta> = Vec::new();
    let mut family_index: HashMap<(String, String), u32> = HashMap::new();
    let mut family_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut tuples: Vec<String> = Vec::new();
    let mut tuple_index: HashMap<String, u32> = HashMap::new();
    let mut combos: Vec<String> = Vec::new();
    let mut combo_index: HashMap<String, u32> = HashMap::new();
    let mut candidates: Vec<CompactCandidate> = Vec::new();
    let mut shape_builders: HashMap<(String, u8), ShapeSlot> = HashMap::new();

    for family in &catalog.families {
        let set = family
            .source_set
            .clone()
            .unwrap_or_else(|| catalog.set.clone());
        family_sets
            .entry(family.family_id.clone())
            .or_default()
            .insert(set.clone());

        let family_idx = if let Some(&i) = family_index.get(&(family.family_id.clone(), set.clone())) {
            i
        } else {
            let i = families.len() as u32;
            families.push(FamilyMeta {
                family_id: family.family_id.clone(),
                set: set.clone(),
                faction: family.faction.clone(),
                family_number: family.family_number.clone(),
            });
            family_index.insert((family.family_id.clone(), set), i);
            i
        };

        let end = family.start_bit + family.max_unique_id;
        for card_index in family.start_bit..end {
            if let Some(pb) = progress {
                pb.inc(1);
            }
            let Some(view) = CompactCardView::from_data(cards_data, card_index) else {
                continue;
            };
            if view.faction_code() == 0 {
                continue;
            }
            let unique_id = card_index - family.start_bit + 1;
            let shape_mask = shape_of(&view);
            let tuple_idx = intern_string(&mut tuples, &mut tuple_index, strict_tuple_id(&strict_tuple(&view)));
            let combo_key = match mode {
                ComboMode::Set => id_gd_set_id(&id_gd_set(&view)),
                ComboMode::Tuple => tuples[tuple_idx as usize].clone(),
                ComboMode::Shape => shape_label(shape_mask),
            };
            let combo_idx = intern_string(&mut combos, &mut combo_index, combo_key);

            let cand_idx = candidates.len();
            candidates.push(CompactCandidate {
                family_idx,
                unique_id,
                shape_mask,
                tuple_idx,
                combo_idx,
            });

            let slot = shape_builders
                .entry((family.family_id.clone(), shape_mask))
                .or_insert_with(|| ShapeSlot {
                    family_id: family.family_id.clone(),
                    shape: shape_mask,
                    tuple_indices: Vec::new(),
                    candidate_indices: Vec::new(),
                    by_tuple: HashMap::new(),
                });
            if !slot.tuple_indices.contains(&tuple_idx) {
                slot.tuple_indices.push(tuple_idx);
            }
            slot.candidate_indices.push(cand_idx);
            slot.by_tuple.entry(tuple_idx).or_default().push(cand_idx);
        }
    }

    let mut shape_slots: Vec<ShapeSlot> = shape_builders.into_values().collect();
    for slot in &mut shape_slots {
        slot.tuple_indices.sort_unstable();
        slot.candidate_indices.sort_unstable_by_key(|&i| {
            let c = &candidates[i];
            let f = &families[c.family_idx as usize];
            (f.set.clone(), c.unique_id)
        });
    }
    shape_slots.sort_by(|a, b| {
        a.family_id
            .cmp(&b.family_id)
            .then_with(|| a.shape.cmp(&b.shape))
    });

    let core_ks_shared: HashSet<String> = family_sets
        .into_iter()
        .filter(|(_, sets)| sets.contains("CORE") && sets.contains("COREKS"))
        .map(|(family_id, _)| family_id)
        .collect();

    Ok(CandidateStore {
        families,
        combos,
        candidates,
        shape_slots,
        core_ks_shared,
    })
}

fn intern_string(pool: &mut Vec<String>, index: &mut HashMap<String, u32>, s: String) -> u32 {
    if let Some(&i) = index.get(&s) {
        return i;
    }
    let i = pool.len() as u32;
    index.insert(s.clone(), i);
    pool.push(s);
    i
}

fn card_identity(store: &CandidateStore, idx: usize) -> CardIdentity {
    let c = &store.candidates[idx];
    let f = &store.families[c.family_idx as usize];
    CardIdentity {
        reference: format!(
            "ALT_{}_B_{}_{}_U_{}",
            f.set, f.faction, f.family_number, c.unique_id
        ),
        set: f.set.clone(),
        faction: f.faction.clone(),
        family_id: f.family_id.clone(),
        family_number: f.family_number.clone(),
        unique_id: c.unique_id,
        shape: shape_label(c.shape_mask),
        combo_id: store.combos[c.combo_idx as usize].clone(),
    }
}

fn record_pick(
    idx: usize,
    store: &CandidateStore,
    picked: &mut [bool],
    used_tuples: &mut HashSet<u32>,
    family_picks: &mut HashMap<String, usize>,
    set_picks: &mut HashMap<(String, String), usize>,
    shape_pick_counts: &mut HashMap<(String, u8), usize>,
    chosen_order: &mut Vec<usize>,
) {
    picked[idx] = true;
    let c = &store.candidates[idx];
    let f = &store.families[c.family_idx as usize];
    used_tuples.insert(c.tuple_idx);
    *family_picks.entry(f.family_id.clone()).or_insert(0) += 1;
    if store.core_ks_shared.contains(&f.family_id) {
        *set_picks
            .entry((f.family_id.clone(), f.set.clone()))
            .or_insert(0) += 1;
    }
    *shape_pick_counts
        .entry((f.family_id.clone(), c.shape_mask))
        .or_insert(0) += 1;
    chosen_order.push(idx);
}

fn best_bucket_pick(
    store: &CandidateStore,
    bucket: &[usize],
    shape_pick_counts: &HashMap<(String, u8), usize>,
    logical_family_id: &str,
    set_picks: &HashMap<(String, String), usize>,
) -> usize {
    let mut best_pos = 0usize;
    let mut best_key = pick_score(
        store,
        bucket[0],
        shape_pick_counts,
        logical_family_id,
        set_picks,
    );
    for (pos, &i) in bucket.iter().enumerate().skip(1) {
        let key = pick_score(store, i, shape_pick_counts, logical_family_id, set_picks);
        if key < best_key {
            best_key = key;
            best_pos = pos;
        }
    }
    best_pos
}

fn pick_score(
    store: &CandidateStore,
    idx: usize,
    shape_pick_counts: &HashMap<(String, u8), usize>,
    logical_family_id: &str,
    set_picks: &HashMap<(String, String), usize>,
) -> (usize, usize, u32) {
    let c = &store.candidates[idx];
    let f = &store.families[c.family_idx as usize];
    let shape_count = shape_pick_counts
        .get(&(f.family_id.clone(), c.shape_mask))
        .copied()
        .unwrap_or(0);
    let set_count = if store.core_ks_shared.contains(logical_family_id) {
        set_picks
            .get(&(f.family_id.clone(), f.set.clone()))
            .copied()
            .unwrap_or(0)
    } else {
        0
    };
    (shape_count, set_count, c.unique_id)
}

fn build_shape_slots_from_index(index_dir: &Path, set: &str) -> Result<(Vec<ShapeSlot>, Vec<FamilyMeta>)> {
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards_data = std::fs::read(set_dir.join("cards.bin"))?;
    let mut families: Vec<FamilyMeta> = Vec::new();
    let mut family_index: HashMap<(String, String), u32> = HashMap::new();
    let mut tuples: Vec<String> = Vec::new();
    let mut tuple_index: HashMap<String, u32> = HashMap::new();
    let mut shape_builders: HashMap<(String, u8), ShapeSlot> = HashMap::new();

    for family in &catalog.families {
        let set_label = family
            .source_set
            .clone()
            .unwrap_or_else(|| catalog.set.clone());
        if !family_index.contains_key(&(family.family_id.clone(), set_label.clone())) {
            let i = families.len() as u32;
            families.push(FamilyMeta {
                family_id: family.family_id.clone(),
                set: set_label.clone(),
                faction: family.faction.clone(),
                family_number: family.family_number.clone(),
            });
            family_index.insert((family.family_id.clone(), set_label), i);
        }

        for ci in family.start_bit..family.start_bit + family.max_unique_id {
            let Some(view) = CompactCardView::from_data(&cards_data, ci) else {
                continue;
            };
            if view.faction_code() == 0 {
                continue;
            }
            let shape = shape_of(&view);
            let tuple_idx = intern_string(
                &mut tuples,
                &mut tuple_index,
                strict_tuple_id(&strict_tuple(&view)),
            );
            let slot = shape_builders
                .entry((family.family_id.clone(), shape))
                .or_insert_with(|| ShapeSlot {
                    family_id: family.family_id.clone(),
                    shape,
                    tuple_indices: Vec::new(),
                    candidate_indices: Vec::new(),
                    by_tuple: HashMap::new(),
                });
            if !slot.tuple_indices.contains(&tuple_idx) {
                slot.tuple_indices.push(tuple_idx);
            }
        }
    }

    let mut shape_slots: Vec<ShapeSlot> = shape_builders.into_values().collect();
    for slot in &mut shape_slots {
        slot.tuple_indices.sort_unstable();
    }
    shape_slots.sort_by(|a, b| {
        a.family_id
            .cmp(&b.family_id)
            .then_with(|| a.shape.cmp(&b.shape))
    });
    Ok((shape_slots, families))
}

fn count_shape_slots(catalog: &Catalog, cards_data: &[u8]) -> usize {
    let mut per_family: HashMap<String, HashSet<u8>> = HashMap::new();
    for family in &catalog.families {
        let shapes = per_family.entry(family.family_id.clone()).or_default();
        for ci in family.start_bit..family.start_bit + family.max_unique_id {
            let Some(view) = CompactCardView::from_data(cards_data, ci) else {
                continue;
            };
            if view.faction_code() == 0 {
                continue;
            }
            shapes.insert(shape_of(&view));
        }
    }
    per_family.values().map(|s| s.len()).sum()
}

struct TupleAssignmentPlan {
    assigned: Vec<(ShapeSlot, u32)>,
    /// Shape slots where every strict tuple collides with another slot; phase 1 picks anyway.
    tuple_collisions: Vec<ShapeSlot>,
}

fn assign_shape_tuples_from_slots(slots: &[ShapeSlot], families: &[FamilyMeta]) -> TupleAssignmentPlan {
    let mut ordered: Vec<&ShapeSlot> = slots.iter().collect();
    ordered.sort_by(|a, b| {
        a.tuple_indices
            .len()
            .cmp(&b.tuple_indices.len())
            .then_with(|| a.family_id.cmp(&b.family_id))
            .then_with(|| a.shape.cmp(&b.shape))
    });
    greedy_tuple_assignment(ordered, families)
}

fn greedy_tuple_assignment(ordered: Vec<&ShapeSlot>, _families: &[FamilyMeta]) -> TupleAssignmentPlan {
    let mut used: HashSet<u32> = HashSet::new();
    let mut assigned: Vec<(ShapeSlot, u32)> = Vec::new();
    let mut tuple_collisions: Vec<ShapeSlot> = Vec::new();
    for slot in ordered {
        if let Some(tuple_idx) = slot.tuple_indices.iter().find(|t| !used.contains(t)).copied() {
            used.insert(tuple_idx);
            assigned.push(((*slot).clone(), tuple_idx));
        } else {
            tuple_collisions.push((*slot).clone());
        }
    }
    TupleAssignmentPlan {
        assigned,
        tuple_collisions,
    }
}

fn pick_shape_floor_card(
    store: &CandidateStore,
    slot: &ShapeSlot,
    preferred_tuple_idx: u32,
    used_tuples: &HashSet<u32>,
    set_picks: &HashMap<(String, String), usize>,
    rng: &mut ChaCha8Rng,
) -> Option<usize> {
    let mut tuple_order: Vec<u32> = Vec::new();
    if !used_tuples.contains(&preferred_tuple_idx) {
        tuple_order.push(preferred_tuple_idx);
    }
    for &tid in &slot.tuple_indices {
        if tid != preferred_tuple_idx && !used_tuples.contains(&tid) {
            tuple_order.push(tid);
        }
    }
    for tuple_idx in tuple_order {
        let Some(candidates) = slot.by_tuple.get(&tuple_idx) else {
            continue;
        };
        if candidates.is_empty() {
            continue;
        }
        return pick_with_set_balance(store, candidates, set_picks, rng);
    }
    None
}

/// Phase-1 pick for shape slots with no unique strict tuple: any random candidate, duplicates allowed.
fn pick_shape_floor_card_allow_duplicate(
    store: &CandidateStore,
    slot: &ShapeSlot,
    picked: &[bool],
    set_picks: &HashMap<(String, String), usize>,
    rng: &mut ChaCha8Rng,
) -> Option<usize> {
    let available: Vec<usize> = slot
        .candidate_indices
        .iter()
        .copied()
        .filter(|&i| !picked[i])
        .collect();
    pick_with_set_balance(store, &available, set_picks, rng)
}

/// Prefer CORE vs COREKS evenly for shared `family_id` values.
fn pick_with_set_balance(
    store: &CandidateStore,
    candidates: &[usize],
    set_picks: &HashMap<(String, String), usize>,
    rng: &mut ChaCha8Rng,
) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    let family_id = &store.families[store.candidates[candidates[0]].family_idx as usize].family_id;
    if !store.core_ks_shared.contains(family_id) {
        return Some(candidates[rng.gen_range(0..candidates.len())]);
    }
    let mut min_set_picks = usize::MAX;
    for &idx in candidates {
        let set = &store.families[store.candidates[idx].family_idx as usize].set;
        let count = set_picks
            .get(&(family_id.clone(), set.clone()))
            .copied()
            .unwrap_or(0);
        min_set_picks = min_set_picks.min(count);
    }
    let tied: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&idx| {
            let set = &store.families[store.candidates[idx].family_idx as usize].set;
            set_picks
                .get(&(family_id.clone(), set.clone()))
                .copied()
                .unwrap_or(0)
                == min_set_picks
        })
        .collect();
    Some(tied[rng.gen_range(0..tied.len())])
}

fn availability_for(
    store: &CandidateStore,
    idx: usize,
    equinox_root: &Path,
    cache: &mut HashMap<String, LocaleAvailability>,
) -> LocaleAvailability {
    let card = card_identity(store, idx);
    if let Some(a) = cache.get(&card.reference) {
        return a.clone();
    }
    let path = card_json_path(
        equinox_root,
        &card.set,
        &card.faction,
        &card.family_number,
        &card.reference,
    );
    let avail = load_locale_availability(&path).unwrap_or_default();
    cache.insert(card.reference.clone(), avail.clone());
    avail
}

#[allow(clippy::too_many_arguments)]
fn build_summary(
    opts: &SampleOptions,
    shape_minimum: usize,
    total_valid: u64,
    distinct_family_ids: usize,
    shape_slots_required: usize,
    shape_slots_achievable: usize,
    shape_slots_covered: usize,
    shape_slots_uncovered: Vec<UncoveredShapeSlot>,
    shape_slots_unpicked: Vec<UncoveredShapeSlot>,
    cards_skipped_no_en: usize,
    distinct_tuples_picked: usize,
    picks_phase1: usize,
    picks_phase2: usize,
    cards_picked: usize,
    plan_cards: usize,
    image_slots_planned: usize,
    by_set: BTreeMap<String, usize>,
    family_picks: BTreeMap<String, usize>,
    by_locale: BTreeMap<String, usize>,
    by_tier: BTreeMap<String, usize>,
) -> SampleSummary {
    let families_total = family_picks.len();
    let mut family_vec: Vec<FamilyPickCount> = family_picks
        .iter()
        .map(|(k, v)| FamilyPickCount {
            family_id: k.clone(),
            set: String::new(),
            picks: *v,
        })
        .collect();
    family_vec.sort_by(|a, b| b.picks.cmp(&a.picks).then_with(|| a.family_id.cmp(&b.family_id)));
    let top20: Vec<FamilyPickCount> = family_vec.into_iter().take(20).collect();

    let families_with_picks = family_picks.values().filter(|v| **v > 0).count();
    let min_picks = family_picks.values().copied().min().unwrap_or(0);
    let max_picks = family_picks.values().copied().max().unwrap_or(0);
    let mean_picks = if families_with_picks == 0 {
        0.0
    } else {
        family_picks.values().copied().sum::<usize>() as f64 / families_with_picks as f64
    };

    SampleSummary {
        index_dir: opts.index_dir.display().to_string(),
        set: opts.set.clone(),
        seed: opts.seed,
        combo_mode: match opts.combo_mode {
            ComboMode::Set => "set".to_string(),
            ComboMode::Tuple => "tuple".to_string(),
            ComboMode::Shape => "shape".to_string(),
        },
        budget: opts.budget,
        shape_coverage_minimum: shape_minimum,
        locale_policy: LocalePolicySummary {
            full_fraction: opts.locale_policy.full_fraction,
            fr_fraction: opts.locale_policy.fr_fraction,
        },
        by_tier,
        image_slots_planned,
        total_valid_cards: total_valid,
        distinct_family_ids,
        shape_slots_required,
        shape_slots_achievable,
        shape_slots_covered,
        shape_slots_uncovered,
        shape_slots_unpicked,
        cards_skipped_no_en,
        distinct_tuples_picked,
        picks_phase1_shape_coverage: picks_phase1,
        picks_phase2_family_quota: picks_phase2,
        cards_picked,
        plan_cards,
        by_set,
        by_family_top20: top20,
        by_family_distribution: FamilyDistribution {
            families_total,
            families_with_picks,
            min_picks,
            max_picks,
            mean_picks,
        },
        by_locale,
    }
}

fn write_summary(path: &Path, summary: &SampleSummary) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    let text = serde_json::to_string_pretty(summary)?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn print_summary(summary: &SampleSummary) {
    println!("== sample ({}/{}) ==", summary.index_dir, summary.set);
    println!(
        "  seed={}, combo_mode={}, budget={}, shape_minimum={}",
        summary.seed, summary.combo_mode, summary.budget, summary.shape_coverage_minimum
    );
    println!(
        "  locale_policy: full={:.2}%, fr={:.2}%",
        summary.locale_policy.full_fraction * 100.0,
        summary.locale_policy.fr_fraction * 100.0
    );
    println!(
        "  valid_cards={}, distinct_family_ids={}",
        summary.total_valid_cards, summary.distinct_family_ids
    );
    println!(
        "  shape_slots: {}/{} covered ({} achievable with unique tuples), distinct_tuples_picked={}",
        summary.shape_slots_covered,
        summary.shape_slots_required,
        summary.shape_slots_achievable,
        summary.distinct_tuples_picked
    );
    if !summary.shape_slots_uncovered.is_empty() {
        println!("  shape slots picked with duplicate strict tuple:");
        for u in &summary.shape_slots_uncovered {
            println!("    {} {}", u.family_id, u.shape);
        }
    }
    if !summary.shape_slots_unpicked.is_empty() {
        println!("  shape slots with no candidate card:");
        for u in &summary.shape_slots_unpicked {
            println!("    {} {}", u.family_id, u.shape);
        }
    }
    if summary.cards_skipped_no_en > 0 {
        println!(
            "  cards skipped (no en_US in equinox JSON): {}",
            summary.cards_skipped_no_en
        );
    }
    println!(
        "  picks: phase1(shape_coverage)={}, phase2(family_quota)={}, total={}",
        summary.picks_phase1_shape_coverage,
        summary.picks_phase2_family_quota,
        summary.cards_picked
    );
    println!(
        "  plan_cards={}, image_slots_planned={}",
        summary.plan_cards, summary.image_slots_planned
    );
    println!();
    println!("  by tier:");
    for (tier, count) in &summary.by_tier {
        println!("    {:<10} {}", tier, count);
    }
    println!();
    println!("  by set:");
    for (set, count) in &summary.by_set {
        println!("    {:<10} {}", set, count);
    }
    println!();
    let d = &summary.by_family_distribution;
    println!(
        "  family distribution: total={}, with_picks={}, min={}, max={}, mean={:.2}",
        d.families_total, d.families_with_picks, d.min_picks, d.max_picks, d.mean_picks
    );
    println!();
    println!("  by locale:");
    for (locale, count) in &summary.by_locale {
        println!("    {:<8} {}", locale, count);
    }
}
