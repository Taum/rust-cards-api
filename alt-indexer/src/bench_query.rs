use crate::bitmap::EffectLine;
use crate::bitmap::BitmapStore;
use crate::catalog::Catalog;
use crate::compact::CompactCardView;
use crate::idgd_catalog::IdGdCatalog;
use crate::query::{execute_idgd_query_preloaded, IdGdQueryBuckets};
use anyhow::{Context, Result};
use roaring::RoaringBitmap;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub queries: usize,
    pub seed: Option<u64>,
    pub warmup: usize,
    pub multi_ids: Option<(usize, usize)>,
    pub json_out: Option<PathBuf>,
    pub print_samples: Option<usize>,
    pub whole_card: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct EffectTuple {
    trigger: u16,
    condition: u16,
    output: u16,
}

#[derive(Debug, Clone, Serialize)]
struct DecodedCardObject {
    reference: String,
    hand: u8,
    reserve: u8,
    m: u8,
    o: u8,
    f: u8,
    main_effects: [EffectTuple; 3],
    echo_effect: EffectTuple,
}

#[derive(Debug, Serialize)]
struct StatsSummary {
    count: usize,
    min_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    mean_ms: f64,
    max_ms: f64,
}

#[derive(Debug, Serialize)]
struct BenchReportJson {
    set: String,
    queries: usize,
    warmup: usize,
    seed: u64,
    mode: String,
    whole_card: bool,
    pool_len: usize,
    multi_ids: Option<(usize, usize)>,
    ops: BenchReportOpsJson,
}

#[derive(Debug, Serialize)]
struct BenchReportOpsJson {
    count: StatsSummary,
    first_50: StatsSummary,
    offset_10000_50: StatsSummary,
}

#[derive(Debug, Clone)]
struct CatalogIdEntry {
    id_gd: u32,
    element_type: String,
}

struct PreloadedIndex {
    per_line: BTreeMap<(u32, EffectLine), RoaringBitmap>,
    whole_card: BTreeMap<u32, RoaringBitmap>,
    catalog_ids: Vec<CatalogIdEntry>,
}

pub fn run(index_dir: &Path, set: &str, opts: BenchOptions) -> Result<()> {
    anyhow::ensure!(opts.queries > 0, "--queries must be > 0");
    if let Some((min, max)) = opts.multi_ids {
        anyhow::ensure!(min > 0 && max > 0, "--multi-ids values must be >= 1");
        anyhow::ensure!(min <= max, "--multi-ids MIN must be <= MAX");
    }

    let set_dir = index_dir.join(set);

    let catalog = Catalog::load(&set_dir.join("catalog.json"))
        .with_context(|| format!("load catalog for set {}", set_dir.display()))?;

    let cards_bin_path = set_dir.join("cards.bin");
    let cards_data =
        std::fs::read(&cards_bin_path).with_context(|| format!("read {}", cards_bin_path.display()))?;

    let idgd_catalog_path = set_dir.join("idgd_catalog.json");
    let idgd_catalog_text = std::fs::read_to_string(&idgd_catalog_path)
        .with_context(|| format!("read {}", idgd_catalog_path.display()))?;
    let idgd_catalog: IdGdCatalog = serde_json::from_str(&idgd_catalog_text)
        .with_context(|| format!("parse {}", idgd_catalog_path.display()))?;

    let index = preload_index(&set_dir, &idgd_catalog, opts.whole_card)?;
    anyhow::ensure!(
        !index.catalog_ids.is_empty(),
        "no idGd values found to benchmark under {}",
        set_dir.join("id_gd").display()
    );

    let seed = opts.seed.unwrap_or_else(randomish_seed_u64);
    let mut rng = XorShift64Star::new(seed);

    if let Some((min, max)) = opts.multi_ids {
        anyhow::ensure!(
            min <= index.catalog_ids.len() && max <= index.catalog_ids.len(),
            "--multi-ids range {min}-{max} exceeds catalog id count {}",
            index.catalog_ids.len()
        );
    }

    if let Some(n) = opts.print_samples {
        println!("sample queries (first {n}):");
        for _ in 0..n {
            if let Some((min, max)) = opts.multi_ids {
                let k = min + rng.next_usize(max - min + 1);
                let buckets = sample_buckets(&mut rng, &index.catalog_ids, k);
                let bmp = run_query(&index, &buckets, opts.whole_card);
                println!(
                    "sample multi query: k={k} triggers={} conditions={} outputs={} (cardinality={})",
                    buckets.triggers.len(),
                    buckets.conditions.len(),
                    buckets.outputs.len(),
                    bmp.len()
                );
            } else {
                let entry = &index.catalog_ids[rng.next_usize(index.catalog_ids.len())];
                let buckets = buckets_for_single_id(entry);
                let bmp = run_query(&index, &buckets, opts.whole_card);
                println!(
                    "sample single query: idGd={} type={} (cardinality={})",
                    entry.id_gd,
                    entry.element_type,
                    bmp.len()
                );
            }
        }
    }

    for _ in 0..opts.warmup {
        let bmp = if let Some((min, max)) = opts.multi_ids {
            let k = min + rng.next_usize(max - min + 1);
            let buckets = sample_buckets(&mut rng, &index.catalog_ids, k);
            run_query(&index, &buckets, opts.whole_card)
        } else {
            let entry = &index.catalog_ids[rng.next_usize(index.catalog_ids.len())];
            let buckets = buckets_for_single_id(entry);
            run_query(&index, &buckets, opts.whole_card)
        };
        let _ = op_count(&bmp);
        let _ = op_list_first_50(&catalog, &cards_data, &bmp)?;
        let _ = op_list_offset_10000_50(&catalog, &cards_data, &bmp)?;
    }

    let mut count_ns: Vec<u64> = Vec::with_capacity(opts.queries);
    let mut first_50_ns: Vec<u64> = Vec::with_capacity(opts.queries);
    let mut offset_ns: Vec<u64> = Vec::with_capacity(opts.queries);

    for _ in 0..opts.queries {
        let bmp = if let Some((min, max)) = opts.multi_ids {
            let k = min + rng.next_usize(max - min + 1);
            let buckets = sample_buckets(&mut rng, &index.catalog_ids, k);
            run_query(&index, &buckets, opts.whole_card)
        } else {
            let entry = &index.catalog_ids[rng.next_usize(index.catalog_ids.len())];
            let buckets = buckets_for_single_id(entry);
            run_query(&index, &buckets, opts.whole_card)
        };

        let t0 = Instant::now();
        let _count = op_count(&bmp);
        count_ns.push(t0.elapsed().as_nanos() as u64);

        let t1 = Instant::now();
        let _cards = op_list_first_50(&catalog, &cards_data, &bmp)?;
        first_50_ns.push(t1.elapsed().as_nanos() as u64);

        let t2 = Instant::now();
        let _cards = op_list_offset_10000_50(&catalog, &cards_data, &bmp)?;
        offset_ns.push(t2.elapsed().as_nanos() as u64);
    }

    let count_summary = summarize_ns(&mut count_ns);
    let first_50_summary = summarize_ns(&mut first_50_ns);
    let offset_summary = summarize_ns(&mut offset_ns);

    print_report(set, seed, &opts, index.catalog_ids.len(), &count_summary, &first_50_summary, &offset_summary);

    if let Some(path) = &opts.json_out {
        let report = BenchReportJson {
            set: set.to_string(),
            queries: opts.queries,
            warmup: opts.warmup,
            seed,
            mode: if opts.multi_ids.is_some() {
                "multi".to_string()
            } else {
                "single".to_string()
            },
            whole_card: opts.whole_card,
            pool_len: index.catalog_ids.len(),
            multi_ids: opts.multi_ids,
            ops: BenchReportOpsJson {
                count: count_summary,
                first_50: first_50_summary,
                offset_10000_50: offset_summary,
            },
        };
        let text = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    }

    Ok(())
}

fn preload_index(
    set_dir: &Path,
    idgd_catalog: &IdGdCatalog,
    whole_card_mode: bool,
) -> Result<PreloadedIndex> {
    let id_gd_dir = set_dir.join("id_gd");
    let mut per_line = BTreeMap::new();
    let mut whole_card = BTreeMap::new();
    let mut catalog_ids = Vec::new();

    for e in &idgd_catalog.entries {
        let element_type = e.element_type.clone();
        let mut has_bitmap = false;

        if whole_card_mode {
            let path = id_gd_dir.join(format!("{}.roar", e.id_gd));
            if path.exists() {
                let bmp = BitmapStore::load(e.id_gd, &path)
                    .with_context(|| format!("load {}", path.display()))?;
                if !bmp.is_empty() {
                    whole_card.insert(e.id_gd, bmp);
                    has_bitmap = true;
                }
            }
        } else {
            for line in EffectLine::ALL {
                let path = id_gd_dir.join(format!("{}_{}.roar", e.id_gd, line.suffix()));
                if !path.exists() {
                    continue;
                }
                let bmp = BitmapStore::load(e.id_gd, &path)
                    .with_context(|| format!("load {}", path.display()))?;
                if !bmp.is_empty() {
                    per_line.insert((e.id_gd, line), bmp);
                    has_bitmap = true;
                }
            }
        }

        if has_bitmap {
            catalog_ids.push(CatalogIdEntry {
                id_gd: e.id_gd,
                element_type,
            });
        }
    }

    Ok(PreloadedIndex {
        per_line,
        whole_card,
        catalog_ids,
    })
}

fn run_query(index: &PreloadedIndex, buckets: &IdGdQueryBuckets, whole_card: bool) -> RoaringBitmap {
    execute_idgd_query_preloaded(
        &index.per_line,
        &index.whole_card,
        buckets,
        whole_card,
    )
}

fn buckets_for_single_id(entry: &CatalogIdEntry) -> IdGdQueryBuckets {
    let mut buckets = IdGdQueryBuckets::default();
    match entry.element_type.as_str() {
        "TRIGGER" => buckets.triggers.push(entry.id_gd),
        "CONDITION" => buckets.conditions.push(entry.id_gd),
        "OUTPUT" => buckets.outputs.push(entry.id_gd),
        _ => {}
    }
    buckets
}

fn sample_buckets(
    rng: &mut XorShift64Star,
    catalog_ids: &[CatalogIdEntry],
    k: usize,
) -> IdGdQueryBuckets {
    let mut picked: Vec<usize> = Vec::with_capacity(k);
    while picked.len() < k {
        let idx = rng.next_usize(catalog_ids.len());
        if !picked.contains(&idx) {
            picked.push(idx);
        }
    }

    let mut buckets = IdGdQueryBuckets::default();
    for idx in picked {
        let entry = &catalog_ids[idx];
        match entry.element_type.as_str() {
            "TRIGGER" => {
                if !buckets.triggers.contains(&entry.id_gd) {
                    buckets.triggers.push(entry.id_gd);
                }
            }
            "CONDITION" => {
                if !buckets.conditions.contains(&entry.id_gd) {
                    buckets.conditions.push(entry.id_gd);
                }
            }
            "OUTPUT" => {
                if !buckets.outputs.contains(&entry.id_gd) {
                    buckets.outputs.push(entry.id_gd);
                }
            }
            _ => {}
        }
    }
    buckets
}

fn randomish_seed_u64() -> u64 {
    let nanos: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let lo = nanos as u64;
    let hi = (nanos >> 64) as u64;
    lo ^ hi ^ 0xA5A5_A5A5_5A5A_5A5A
}

fn op_count(bitmap: &RoaringBitmap) -> u64 {
    bitmap.len()
}

fn op_list_first_50(catalog: &Catalog, cards_data: &[u8], bitmap: &RoaringBitmap) -> Result<Vec<DecodedCardObject>> {
    let mut out = Vec::with_capacity(50);
    for bit in bitmap.iter().take(50) {
        if let Some(obj) = decode_card_object(catalog, cards_data, bit)? {
            out.push(obj);
        }
    }
    Ok(out)
}

fn op_list_offset_10000_50(
    catalog: &Catalog,
    cards_data: &[u8],
    bitmap: &RoaringBitmap,
) -> Result<Vec<DecodedCardObject>> {
    let card_count = bitmap.len() as u64;
    let start = if card_count >= 10_000 {
        10_000u64
    } else {
        card_count.saturating_sub(50)
    };

    let mut out = Vec::with_capacity(50);
    for bit in bitmap.iter().skip(start as usize).take(50) {
        if let Some(obj) = decode_card_object(catalog, cards_data, bit)? {
            out.push(obj);
        }
    }
    Ok(out)
}

fn decode_card_object(catalog: &Catalog, cards_data: &[u8], bit: u32) -> Result<Option<DecodedCardObject>> {
    let decoded = catalog.decode_bit(bit)?;
    let view = match CompactCardView::from_data(cards_data, bit) {
        Some(v) => v,
        None => return Ok(None),
    };

    let main_effects = [
        effect_tuple(view.main_effect_group(0)),
        effect_tuple(view.main_effect_group(1)),
        effect_tuple(view.main_effect_group(2)),
    ];
    let echo_effect = effect_tuple(view.echo_effect());

    Ok(Some(DecodedCardObject {
        reference: decoded.reference,
        hand: view.main_cost(),
        reserve: view.recall_cost(),
        m: view.mountain_power(),
        o: view.ocean_power(),
        f: view.forest_power(),
        main_effects,
        echo_effect,
    }))
}

fn effect_tuple(raw: [u16; 3]) -> EffectTuple {
    EffectTuple {
        trigger: raw[0],
        condition: raw[1],
        output: raw[2],
    }
}

fn summarize_ns(values: &mut [u64]) -> StatsSummary {
    values.sort_unstable();
    let count = values.len().max(1);
    let min_ns = values.first().copied().unwrap_or(0);
    let max_ns = values.last().copied().unwrap_or(0);
    let p50_ns = values[(count - 1) * 50 / 100];
    let p95_ns = values[(count - 1) * 95 / 100];
    let sum: u128 = values.iter().map(|&v| v as u128).sum();
    let mean_ns = (sum / (count as u128)) as u64;
    StatsSummary {
        count: values.len(),
        min_ms: ns_to_ms(min_ns),
        p50_ms: ns_to_ms(p50_ns),
        p95_ms: ns_to_ms(p95_ns),
        mean_ms: ns_to_ms(mean_ns),
        max_ms: ns_to_ms(max_ns),
    }
}

fn ns_to_ms(ns: u64) -> f64 {
    (ns as f64) / 1_000_000.0
}

fn print_report(
    set: &str,
    seed: u64,
    opts: &BenchOptions,
    pool_len: usize,
    count: &StatsSummary,
    first_50: &StatsSummary,
    offset: &StatsSummary,
) {
    let index_mode = if opts.whole_card { "whole-card" } else { "per-line" };
    if let Some((min, max)) = opts.multi_ids {
        println!(
            "bench-query: set={set} seed={seed} index={index_mode} mode=multi multi_ids={min}-{max} warmup={} queries={}",
            opts.warmup, opts.queries
        );
    } else {
        println!(
            "bench-query: set={set} seed={seed} index={index_mode} mode=single warmup={} queries={} pool_len={pool_len}",
            opts.warmup, opts.queries
        );
    }
    println!();
    print_op("count", count);
    print_op("first_50", first_50);
    print_op("offset_10000_50", offset);
}

fn print_op(name: &str, s: &StatsSummary) {
    fn fmt_ms(v: f64) -> String {
        format!("{v:.3}ms")
    }

    println!(
        "{name:<16} n={:<6} min={:<12} p50={:<12} p95={:<12} mean={:<12} max={:<12}",
        s.count,
        fmt_ms(s.min_ms),
        fmt_ms(s.p50_ms),
        fmt_ms(s.p95_ms),
        fmt_ms(s.mean_ms),
        fmt_ms(s.max_ms),
    );
}

#[derive(Debug, Clone, Copy)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        (self.next_u64() % (upper as u64)) as usize
    }
}
