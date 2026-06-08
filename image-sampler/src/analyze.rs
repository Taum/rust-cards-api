use crate::combo::{id_gd_set, id_gd_set_id, shape_label, shape_of, strict_tuple, strict_tuple_id};
use crate::progress::{self, StepGuard};
use anyhow::{Context, Result};
use index_core::catalog::Catalog;
use index_core::compact::{CompactCardView, RECORD_SIZE};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeReport {
    pub index_dir: String,
    pub set: String,
    pub total_bit_span: u32,
    pub padding_slots: u64,
    pub valid_cards: u64,
    pub by_set: BTreeMap<String, u64>,
    pub by_family: Vec<FamilyCount>,
    pub shapes: ShapeReport,
    pub id_gd_sets: ComboReport,
    pub strict_tuples: ComboReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilyCount {
    pub set: String,
    pub family_id: String,
    pub valid_cards: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShapeReport {
    pub distinct: usize,
    pub counts: Vec<ShapeCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShapeCount {
    pub label: String,
    pub mask: u8,
    pub cards: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComboReport {
    pub distinct: u64,
    pub singletons: u64,
    pub max_cards_per_combo: u64,
    pub mean_cards_per_combo: f64,
    /// Bucketed histogram of `cards-per-combo` so we can eyeball the long tail.
    pub histogram: Vec<HistogramBucket>,
    /// Expected uniform-random draws to cover every combo with high probability
    /// (coupon-collector ≈ N * (ln N + γ)).
    pub coupon_collector_estimate: u64,
    /// Deterministic minimum: pick one card per combo (sampler does this).
    pub deterministic_minimum: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistogramBucket {
    pub min: u64,
    pub max_inclusive: u64,
    pub combos: u64,
}

pub struct AnalyzeOptions {
    pub index_dir: PathBuf,
    pub set: String,
    pub out_json: Option<PathBuf>,
}

pub fn run(opts: &AnalyzeOptions) -> Result<AnalyzeReport> {
    const P: &str = "analyze";
    let set_dir = opts.index_dir.join(&opts.set);
    let catalog_path = set_dir.join("catalog.json");
    let cards_bin_path = set_dir.join("cards.bin");

    let (catalog, cards_data, expected_len) = {
        let step = StepGuard::begin(P, "load index");
        let catalog = Catalog::load(&catalog_path)
            .with_context(|| format!("load catalog {}", catalog_path.display()))?;
        let cards_data = std::fs::read(&cards_bin_path)
            .with_context(|| format!("read {}", cards_bin_path.display()))?;
        let expected_len = catalog.total_bit_span as usize * RECORD_SIZE;
        step.finish(Some(&format!("{} bit slots", catalog.total_bit_span)));
        (catalog, cards_data, expected_len)
    };

    anyhow::ensure!(
        cards_data.len() >= expected_len,
        "cards.bin size {} smaller than expected {} (total_bit_span {} * {})",
        cards_data.len(),
        expected_len,
        catalog.total_bit_span,
        RECORD_SIZE
    );

    let mut valid: u64 = 0;
    let mut padding: u64 = 0;
    let mut by_set: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_family: Vec<FamilyCount> = Vec::with_capacity(catalog.families.len());
    let mut shapes: [u64; 16] = [0; 16];
    let mut combo_set_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut combo_tuple_counts: BTreeMap<String, u64> = BTreeMap::new();

    let scan_pb = progress::bar(P, "scan cards.bin", catalog.total_bit_span as u64);
    for family in &catalog.families {
        let mut fam_valid: u64 = 0;
        let start = family.start_bit;
        let end = family.start_bit + family.max_unique_id;
        for card_index in start..end {
            scan_pb.inc(1);
            let Some(view) = CompactCardView::from_data(&cards_data, card_index) else {
                padding += 1;
                continue;
            };
            if view.faction_code() == 0 {
                padding += 1;
                continue;
            }
            valid += 1;
            fam_valid += 1;
            let shape = shape_of(&view);
            shapes[shape as usize] += 1;
            let set_ids = id_gd_set(&view);
            *combo_set_counts.entry(id_gd_set_id(&set_ids)).or_insert(0) += 1;
            let tuple = strict_tuple(&view);
            *combo_tuple_counts.entry(strict_tuple_id(&tuple)).or_insert(0) += 1;
        }
        let set_label = family
            .source_set
            .clone()
            .unwrap_or_else(|| catalog.set.clone());
        *by_set.entry(set_label.clone()).or_insert(0) += fam_valid;
        by_family.push(FamilyCount {
            set: set_label,
            family_id: family.family_id.clone(),
            valid_cards: fam_valid,
        });
    }
    progress::finish_bar(scan_pb, format!("{valid} valid cards"));

    let report = AnalyzeReport {
        index_dir: opts.index_dir.display().to_string(),
        set: opts.set.clone(),
        total_bit_span: catalog.total_bit_span,
        padding_slots: padding,
        valid_cards: valid,
        by_set,
        by_family,
        shapes: build_shape_report(&shapes),
        id_gd_sets: build_combo_report(&combo_set_counts),
        strict_tuples: build_combo_report(&combo_tuple_counts),
    };

    if let Some(path) = &opts.out_json {
        let step = StepGuard::begin(P, "write report");
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create dir {}", parent.display()))?;
            }
        }
        let text = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
        step.finish(Some(&path.display().to_string()));
    }

    Ok(report)
}

fn build_shape_report(shapes: &[u64; 16]) -> ShapeReport {
    let mut counts: Vec<ShapeCount> = (0u8..16)
        .filter(|m| shapes[*m as usize] > 0)
        .map(|m| ShapeCount {
            label: shape_label(m),
            mask: m,
            cards: shapes[m as usize],
        })
        .collect();
    counts.sort_by(|a, b| b.cards.cmp(&a.cards));
    ShapeReport {
        distinct: counts.len(),
        counts,
    }
}

fn build_combo_report(counts: &BTreeMap<String, u64>) -> ComboReport {
    let distinct = counts.len() as u64;
    let total: u64 = counts.values().sum();
    let singletons = counts.values().filter(|c| **c == 1).count() as u64;
    let max_cards_per_combo = counts.values().copied().max().unwrap_or(0);
    let mean = if distinct == 0 {
        0.0
    } else {
        total as f64 / distinct as f64
    };
    let histogram = build_histogram(counts.values().copied());
    let coupon = coupon_collector_estimate(distinct);
    ComboReport {
        distinct,
        singletons,
        max_cards_per_combo,
        mean_cards_per_combo: mean,
        histogram,
        coupon_collector_estimate: coupon,
        deterministic_minimum: distinct,
    }
}

/// Bucket combos by cards-per-combo (`1`, `2`, `3-5`, `6-10`, `11-100`, `101-1000`, `1001+`).
fn build_histogram(values: impl IntoIterator<Item = u64>) -> Vec<HistogramBucket> {
    let edges: &[(u64, u64)] = &[
        (1, 1),
        (2, 2),
        (3, 5),
        (6, 10),
        (11, 100),
        (101, 1000),
        (1001, u64::MAX),
    ];
    let mut buckets: Vec<HistogramBucket> = edges
        .iter()
        .map(|(lo, hi)| HistogramBucket {
            min: *lo,
            max_inclusive: *hi,
            combos: 0,
        })
        .collect();
    for v in values {
        for b in &mut buckets {
            if v >= b.min && v <= b.max_inclusive {
                b.combos += 1;
                break;
            }
        }
    }
    buckets
}

fn coupon_collector_estimate(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    let nf = n as f64;
    let estimate = nf * (nf.ln() + EULER_GAMMA);
    estimate.ceil() as u64
}

pub fn print_report(report: &AnalyzeReport) {
    println!("== analyze ({}/{}) ==", report.index_dir, report.set);
    println!(
        "  bit slots: {}  valid: {}  padding: {}",
        report.total_bit_span, report.valid_cards, report.padding_slots
    );
    println!();
    println!("  by set:");
    for (set, count) in &report.by_set {
        println!("    {:<10} {}", set, count);
    }
    println!();
    println!("  by shape ({} distinct):", report.shapes.distinct);
    println!("    {:<18} {:>4}  {:>12}", "label", "mask", "cards");
    for s in &report.shapes.counts {
        println!("    {:<18} {:>4}  {:>12}", s.label, s.mask, s.cards);
    }
    println!();
    print_combo_block("id_gd set (canonical)", &report.id_gd_sets);
    println!();
    print_combo_block("strict 12-slot tuple", &report.strict_tuples);
}

fn print_combo_block(label: &str, r: &ComboReport) {
    println!(
        "  {label}: distinct={}, singletons={}, max-cards-per-combo={}, mean={:.2}",
        r.distinct, r.singletons, r.max_cards_per_combo, r.mean_cards_per_combo
    );
    println!(
        "    coverage: {} draws (deterministic, 1-per-combo)  |  {} draws (uniform-random coupon-collector estimate)",
        r.deterministic_minimum, r.coupon_collector_estimate
    );
    println!("    histogram of cards-per-combo:");
    for b in &r.histogram {
        let range = if b.min == b.max_inclusive {
            format!("{}", b.min)
        } else if b.max_inclusive == u64::MAX {
            format!("{}+", b.min)
        } else {
            format!("{}-{}", b.min, b.max_inclusive)
        };
        println!("      {:>10}  {:>10}", range, b.combos);
    }
}
