use crate::bitmap::BitmapStore;
use crate::catalog::Catalog;
use crate::compact::CompactCardView;
use crate::idgd_catalog::IdGdCatalog;
use anyhow::{Context, Result};
use roaring::RoaringBitmap;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub queries: usize,
    pub seed: Option<u64>,
    pub warmup: usize,
    pub multi_ids: Option<(usize, usize)>,
    pub json_out: Option<PathBuf>,
    pub print_samples: Option<usize>,
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

    let seed = opts.seed.unwrap_or_else(randomish_seed_u64);
    let mut rng = XorShift64Star::new(seed);
    let pool = build_query_pool(&set_dir, &idgd_catalog)?;
    anyhow::ensure!(
        !pool.is_empty(),
        "no idGd bitmaps found to benchmark under {}",
        set_dir.join("id_gd").display()
    );

    if let Some((min, max)) = opts.multi_ids {
        anyhow::ensure!(
            min <= pool.len() && max <= pool.len(),
            "--multi-ids range {min}-{max} exceeds pool size {}",
            pool.len()
        );
    }

    if let Some(n) = opts.print_samples {
        println!("sample pool (first {n}):");
        if let Some((min, max)) = opts.multi_ids {
            for _ in 0..n {
                let k = min + rng.next_usize(max - min + 1);
                let sample = sample_multi_query_bitmap(&mut rng, &pool, k);
                println!("sample multi query: k={k} (cardinality={})", sample.len());
            }
        } else {
            for _ in 0..n {
                let idx = rng.next_usize(pool.len());
                let sample = &pool[idx].bitmap;
                println!("sample single query: idx={idx} (cardinality={})", sample.len());
            }
        }
    }

    // Warmup (not recorded).
    for _ in 0..opts.warmup {
        if let Some((min, max)) = opts.multi_ids {
            let k = min + rng.next_usize(max - min + 1);
            let bmp = sample_multi_query_bitmap(&mut rng, &pool, k);
            let _ = op_count(&bmp);
            let _ = op_list_first_50(&catalog, &cards_data, &bmp)?;
            let _ = op_list_offset_10000_50(&catalog, &cards_data, &bmp)?;
        } else {
            let idx = rng.next_usize(pool.len());
            let bmp = &pool[idx].bitmap;
            let _ = op_count(bmp);
            let _ = op_list_first_50(&catalog, &cards_data, bmp)?;
            let _ = op_list_offset_10000_50(&catalog, &cards_data, bmp)?;
        }
    }

    let mut count_ns: Vec<u64> = Vec::with_capacity(opts.queries);
    let mut first_50_ns: Vec<u64> = Vec::with_capacity(opts.queries);
    let mut offset_ns: Vec<u64> = Vec::with_capacity(opts.queries);

    for _ in 0..opts.queries {
        if let Some((min, max)) = opts.multi_ids {
            let k = min + rng.next_usize(max - min + 1);
            let bmp = sample_multi_query_bitmap(&mut rng, &pool, k);

            let t0 = Instant::now();
            let _count = op_count(&bmp);
            count_ns.push(t0.elapsed().as_nanos() as u64);

            let t1 = Instant::now();
            let _cards = op_list_first_50(&catalog, &cards_data, &bmp)?;
            first_50_ns.push(t1.elapsed().as_nanos() as u64);

            let t2 = Instant::now();
            let _cards = op_list_offset_10000_50(&catalog, &cards_data, &bmp)?;
            offset_ns.push(t2.elapsed().as_nanos() as u64);
        } else {
            let idx = rng.next_usize(pool.len());
            let bmp = &pool[idx].bitmap;

            let t0 = Instant::now();
            let _count = op_count(bmp);
            count_ns.push(t0.elapsed().as_nanos() as u64);

            let t1 = Instant::now();
            let _cards = op_list_first_50(&catalog, &cards_data, bmp)?;
            first_50_ns.push(t1.elapsed().as_nanos() as u64);

            let t2 = Instant::now();
            let _cards = op_list_offset_10000_50(&catalog, &cards_data, bmp)?;
            offset_ns.push(t2.elapsed().as_nanos() as u64);
        }
    }

    let count_summary = summarize_ns(&mut count_ns);
    let first_50_summary = summarize_ns(&mut first_50_ns);
    let offset_summary = summarize_ns(&mut offset_ns);

    print_report(set, seed, &opts, &count_summary, &first_50_summary, &offset_summary);

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
            pool_len: pool.len(),
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

fn randomish_seed_u64() -> u64 {
    // Not crypto-random; just a convenient non-deterministic-ish seed.
    let nanos: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let lo = nanos as u64;
    let hi = (nanos >> 64) as u64;
    lo ^ hi ^ 0xA5A5_A5A5_5A5A_5A5A
}

#[derive(Debug, Clone)]
struct PoolEntry {
    element_type: String,
    bitmap: RoaringBitmap,
}

fn build_query_pool(set_dir: &Path, idgd_catalog: &IdGdCatalog) -> Result<Vec<PoolEntry>> {
    let id_gd_dir = set_dir.join("id_gd");
    let mut pool = Vec::new();

    for e in &idgd_catalog.entries {
        let id_gd = e.id_gd;
        let bitmap_path = id_gd_dir.join(format!("{id_gd}.roar"));
        if !bitmap_path.exists() {
            continue;
        }
        let bmp = BitmapStore::load(id_gd, &bitmap_path)
            .with_context(|| format!("load {}", bitmap_path.display()))?;
        if bmp.is_empty() {
            continue;
        }
        pool.push(PoolEntry {
            element_type: e.element_type.clone(),
            bitmap: bmp,
        });
    }

    Ok(pool)
}

fn sample_multi_query_bitmap(rng: &mut XorShift64Star, pool: &[PoolEntry], k: usize) -> RoaringBitmap {
    let mut picked: Vec<usize> = Vec::with_capacity(k);
    while picked.len() < k {
        let idx = rng.next_usize(pool.len());
        if !picked.contains(&idx) {
            picked.push(idx);
        }
    }

    let mut trig: Option<RoaringBitmap> = None;
    let mut cond: Option<RoaringBitmap> = None;
    let mut out: Option<RoaringBitmap> = None;

    for idx in picked {
        let e = &pool[idx];
        match e.element_type.as_str() {
            "TRIGGER" => {
                let acc = trig.get_or_insert_with(RoaringBitmap::new);
                *acc |= &e.bitmap;
            }
            "CONDITION" => {
                let acc = cond.get_or_insert_with(RoaringBitmap::new);
                *acc |= &e.bitmap;
            }
            "OUTPUT" => {
                let acc = out.get_or_insert_with(RoaringBitmap::new);
                *acc |= &e.bitmap;
            }
            _ => {
                // Unknown types are ignored for the benchmark pool; query tool errors on these.
            }
        }
    }

    let mut it = [trig, cond, out].into_iter().flatten();
    let mut bitmap = it.next().unwrap_or_else(RoaringBitmap::new);
    for g in it {
        bitmap &= g;
    }
    bitmap
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

fn print_report(set: &str, seed: u64, opts: &BenchOptions, count: &StatsSummary, first_50: &StatsSummary, offset: &StatsSummary) {
    if let Some((min, max)) = opts.multi_ids {
        println!("bench-query: set={set} seed={seed} mode=multi multi_ids={min}-{max} warmup={} queries={}", opts.warmup, opts.queries);
    } else {
        println!("bench-query: set={set} seed={seed} mode=single warmup={} queries={}", opts.warmup, opts.queries);
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

// --- deterministic RNG (no rand dep) ---

#[derive(Debug, Clone, Copy)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        // Avoid all-zero state (degenerate).
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
        // Mod bias is acceptable for benchmarking selection.
        (self.next_u64() % (upper as u64)) as usize
    }
}

// shuffle_in_place removed (no longer sampling pool-size; we load full pool)

