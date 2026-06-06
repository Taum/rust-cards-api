use index_core::bitmap::EffectLine;
use index_core::bitmap::BitmapStore;
use index_core::catalog::Catalog;
use index_core::compact::CompactCardView;
use index_core::idgd_catalog::IdGdCatalog;
use index_core::progress::MultiIdCombinationProgress;
use index_core::query::{execute_idgd_query_preloaded, IdGdQueryBuckets};
use anyhow::{Context, Result};
use roaring::RoaringBitmap;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAX_PACK_RETRIES: u32 = 64;

#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub queries: usize,
    pub seed: Option<u64>,
    pub warmup: usize,
    pub multi_ids: Option<(usize, usize)>,
    pub json_out: Option<PathBuf>,
    pub print_samples: Option<usize>,
    pub whole_card: bool,
    pub roaring_only: bool,
    pub json_samples: bool,
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
struct CardinalitySummary {
    p50: u64,
    p95: u64,
    min: u64,
    max: u64,
}

#[derive(Debug, Serialize)]
struct BenchReportJson {
    set: String,
    queries: usize,
    warmup: usize,
    seed: u64,
    mode: String,
    whole_card: bool,
    roaring_only: bool,
    pool_len: usize,
    multi_ids: Option<(usize, usize)>,
    sampling_restarts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_combinations_wall_ms: Option<f64>,
    cardinality: CardinalitySummary,
    ops: BenchReportOpsJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    samples: Option<Vec<BenchSampleJson>>,
}

#[derive(Debug, Serialize)]
struct BenchSampleJson {
    cardinality: u64,
    start_rank: u64,
    intersect_ms: f64,
}

#[derive(Debug, Serialize)]
struct BenchReportOpsJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_combinations: Option<StatsSummary>,
    intersect: StatsSummary,
    count: StatsSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_50: Option<StatsSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset_10000_50: Option<StatsSummary>,
    window_skip: StatsSummary,
    window_select: StatsSummary,
    window_advance: StatsSummary,
}

#[derive(Debug, Clone)]
struct CatalogIdEntry {
    id_gd: u32,
    element_type: String,
}

struct CatalogByType {
    triggers: Vec<usize>,
    conditions: Vec<usize>,
    outputs: Vec<usize>,
}

struct PreloadedIndex {
    per_line: BTreeMap<(u32, EffectLine), RoaringBitmap>,
    whole_card: BTreeMap<u32, RoaringBitmap>,
    catalog_ids: Vec<CatalogIdEntry>,
    by_type: CatalogByType,
}

/// Cached bucket unions while building a multi-id pack (avoids repeated full intersects).
struct PackBuilder {
    buckets: IdGdQueryBuckets,
    cache: PackCache,
}

enum PackCache {
    PerLine {
        trigger: BTreeMap<EffectLine, RoaringBitmap>,
        condition: BTreeMap<EffectLine, RoaringBitmap>,
        output: BTreeMap<EffectLine, RoaringBitmap>,
    },
    Whole {
        trigger: RoaringBitmap,
        condition: RoaringBitmap,
        output: RoaringBitmap,
    },
}

struct QueryTimings {
    intersect_ns: u64,
    count_ns: u64,
    first_50_ns: Option<u64>,
    offset_ns: Option<u64>,
    window_skip_ns: u64,
    window_select_ns: u64,
    window_advance_ns: u64,
    cardinality: u64,
    start_rank: u64,
}

pub fn run(index_dir: &Path, set: &str, opts: BenchOptions) -> Result<()> {
    anyhow::ensure!(opts.queries > 0, "--queries must be > 0");
    if let Some((min, max)) = opts.multi_ids {
        anyhow::ensure!(min > 0 && max > 0, "--multi-ids values must be >= 1");
        anyhow::ensure!(min <= max, "--multi-ids MIN must be <= MAX");
    }

    let set_dir = index_dir.join(set);

    let catalog = if opts.roaring_only {
        None
    } else {
        Some(
            Catalog::load(&set_dir.join("catalog.json"))
                .with_context(|| format!("load catalog for set {}", set_dir.display()))?,
        )
    };

    let cards_data = if opts.roaring_only {
        Vec::new()
    } else {
        let cards_bin_path = set_dir.join("cards.bin");
        std::fs::read(&cards_bin_path)
            .with_context(|| format!("read {}", cards_bin_path.display()))?
    };

    let idgd_catalog_path = set_dir.join("idgd_catalog.json");
    let idgd_catalog_text = std::fs::read_to_string(&idgd_catalog_path)
        .with_context(|| format!("read {}", idgd_catalog_path.display()))?;
    let idgd_catalog: IdGdCatalog = serde_json::from_str(&idgd_catalog_text)
        .with_context(|| format!("parse {}", idgd_catalog_path.display()))?;

    let mut index = preload_index(&set_dir, &idgd_catalog, opts.whole_card)?;
    filter_nonempty_solo_ids(&mut index, opts.whole_card);
    anyhow::ensure!(
        !index.catalog_ids.is_empty(),
        "no idGd values with non-empty solo queries under {}",
        set_dir.join("id_gd").display()
    );

    let seed = opts.seed.unwrap_or_else(randomish_seed_u64);
    let mut rng = XorShift64Star::new(seed);
    let mut sampling_restarts: u64 = 0;
    let multi_id_mode = opts.multi_ids.is_some();
    let combination_steps = if multi_id_mode {
        opts.print_samples.unwrap_or(0) + opts.warmup + opts.queries
    } else {
        0
    };
    let mut combination_ns: Vec<u64> = Vec::with_capacity(combination_steps);
    let combination_progress = MultiIdCombinationProgress::start(combination_steps);
    let combination_phase = Instant::now();

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
            let (buckets, restarts, sample_ns) = sample_query_buckets_timed(
                &index,
                &mut rng,
                opts.multi_ids,
                opts.whole_card,
                multi_id_mode,
            )?;
            record_combination_sample(
                multi_id_mode,
                &combination_progress,
                &mut combination_ns,
                &mut sampling_restarts,
                restarts,
                sample_ns,
            );
            let bmp = run_query(&index, &buckets, opts.whole_card);
            println!(
                "sample query: triggers={} conditions={} outputs={} (cardinality={})",
                buckets.triggers.len(),
                buckets.conditions.len(),
                buckets.outputs.len(),
                bmp.len()
            );
        }
    }

    for _ in 0..opts.warmup {
        let (buckets, restarts, sample_ns) = sample_query_buckets_timed(
            &index,
            &mut rng,
            opts.multi_ids,
            opts.whole_card,
            multi_id_mode,
        )?;
        record_combination_sample(
            multi_id_mode,
            &combination_progress,
            &mut combination_ns,
            &mut sampling_restarts,
            restarts,
            sample_ns,
        );
        run_timed_ops(
            &index,
            catalog.as_ref(),
            &cards_data,
            &buckets,
            opts.whole_card,
            opts.roaring_only,
        )?;
    }

    let mut timings: Vec<QueryTimings> = Vec::with_capacity(opts.queries);

    for _ in 0..opts.queries {
        let (buckets, restarts, sample_ns) = sample_query_buckets_timed(
            &index,
            &mut rng,
            opts.multi_ids,
            opts.whole_card,
            multi_id_mode,
        )?;
        record_combination_sample(
            multi_id_mode,
            &combination_progress,
            &mut combination_ns,
            &mut sampling_restarts,
            restarts,
            sample_ns,
        );
        timings.push(run_timed_ops(
            &index,
            catalog.as_ref(),
            &cards_data,
            &buckets,
            opts.whole_card,
            opts.roaring_only,
        )?);
    }

    let combination_wall_ns = if multi_id_mode {
        let elapsed = combination_phase.elapsed();
        combination_progress.finish(elapsed, combination_steps, sampling_restarts);
        Some(elapsed.as_nanos() as u64)
    } else {
        None
    };

    let sample_combinations_summary = multi_id_mode.then(|| {
        let mut ns = combination_ns;
        summarize_ns(&mut ns)
    });

    let summaries = summarize_timings(&timings, opts.roaring_only, sample_combinations_summary);
    let cardinality_summary = summarize_cardinality(&timings);

    print_report(
        set,
        seed,
        &opts,
        index.catalog_ids.len(),
        sampling_restarts,
        combination_wall_ns,
        &cardinality_summary,
        &summaries,
    );

    if let Some(path) = &opts.json_out {
        let report = build_json_report(
            set,
            &opts,
            seed,
            index.catalog_ids.len(),
            sampling_restarts,
            combination_wall_ns,
            cardinality_summary,
            summaries,
            &timings,
        );
        let text = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    }

    Ok(())
}

fn run_timed_ops(
    index: &PreloadedIndex,
    catalog: Option<&Catalog>,
    cards_data: &[u8],
    buckets: &IdGdQueryBuckets,
    whole_card: bool,
    roaring_only: bool,
) -> Result<QueryTimings> {
    let t0 = Instant::now();
    let bmp = run_query(index, buckets, whole_card);
    let intersect_ns = t0.elapsed().as_nanos() as u64;
    anyhow::ensure!(
        !bmp.is_empty(),
        "sampled query must be non-empty (bench-query sampling bug)"
    );

    let cardinality = bmp.len();
    let start_rank = window_start_rank(cardinality);

    let t1 = Instant::now();
    let _count = op_count(&bmp);
    let count_ns = t1.elapsed().as_nanos() as u64;

    let (first_50_ns, offset_ns) = if roaring_only {
        (None, None)
    } else {
        let catalog = catalog.expect("catalog required when not --roaring-only");
        let t2 = Instant::now();
        let _cards = op_list_first_50(catalog, cards_data, &bmp)?;
        let first_50_ns = t2.elapsed().as_nanos() as u64;

        let t3 = Instant::now();
        let _cards = op_list_offset_10000_50(catalog, cards_data, &bmp)?;
        let offset_ns = t3.elapsed().as_nanos() as u64;
        (Some(first_50_ns), Some(offset_ns))
    };

    let t4 = Instant::now();
    let _ = op_window_skip_take(&bmp, start_rank);
    let window_skip_ns = t4.elapsed().as_nanos() as u64;

    let t5 = Instant::now();
    let _ = op_window_select_loop(&bmp, start_rank);
    let window_select_ns = t5.elapsed().as_nanos() as u64;

    let t6 = Instant::now();
    let _ = op_window_select_advance(&bmp, start_rank);
    let window_advance_ns = t6.elapsed().as_nanos() as u64;

    Ok(QueryTimings {
        intersect_ns,
        count_ns,
        first_50_ns,
        offset_ns,
        window_skip_ns,
        window_select_ns,
        window_advance_ns,
        cardinality,
        start_rank,
    })
}

struct OpSummaries {
    sample_combinations: Option<StatsSummary>,
    intersect: StatsSummary,
    count: StatsSummary,
    first_50: Option<StatsSummary>,
    offset_10000_50: Option<StatsSummary>,
    window_skip: StatsSummary,
    window_select: StatsSummary,
    window_advance: StatsSummary,
}

fn record_combination_sample(
    multi_id_mode: bool,
    progress: &MultiIdCombinationProgress,
    combination_ns: &mut Vec<u64>,
    sampling_restarts: &mut u64,
    restarts: u64,
    sample_ns: Option<u64>,
) {
    *sampling_restarts += restarts;
    if multi_id_mode {
        if let Some(ns) = sample_ns {
            combination_ns.push(ns);
        }
        progress.inc(*sampling_restarts);
    }
}

fn sample_query_buckets_timed(
    index: &PreloadedIndex,
    rng: &mut XorShift64Star,
    multi_ids: Option<(usize, usize)>,
    whole_card: bool,
    time_sample: bool,
) -> Result<(IdGdQueryBuckets, u64, Option<u64>)> {
    let t0 = Instant::now();
    let (buckets, restarts) = sample_query_buckets(index, rng, multi_ids, whole_card)?;
    let sample_ns = time_sample.then(|| t0.elapsed().as_nanos() as u64);
    Ok((buckets, restarts, sample_ns))
}

fn summarize_timings(
    timings: &[QueryTimings],
    roaring_only: bool,
    sample_combinations: Option<StatsSummary>,
) -> OpSummaries {
    let mut intersect_ns: Vec<u64> = timings.iter().map(|t| t.intersect_ns).collect();
    let mut count_ns: Vec<u64> = timings.iter().map(|t| t.count_ns).collect();
    let mut window_skip_ns: Vec<u64> = timings.iter().map(|t| t.window_skip_ns).collect();
    let mut window_select_ns: Vec<u64> = timings.iter().map(|t| t.window_select_ns).collect();
    let mut window_advance_ns: Vec<u64> = timings.iter().map(|t| t.window_advance_ns).collect();

    let (first_50, offset_10000_50) = if roaring_only {
        (None, None)
    } else {
        let mut first_50_ns: Vec<u64> = timings
            .iter()
            .filter_map(|t| t.first_50_ns)
            .collect();
        let mut offset_ns: Vec<u64> = timings.iter().filter_map(|t| t.offset_ns).collect();
        (
            Some(summarize_ns(&mut first_50_ns)),
            Some(summarize_ns(&mut offset_ns)),
        )
    };

    OpSummaries {
        sample_combinations,
        intersect: summarize_ns(&mut intersect_ns),
        count: summarize_ns(&mut count_ns),
        first_50,
        offset_10000_50,
        window_skip: summarize_ns(&mut window_skip_ns),
        window_select: summarize_ns(&mut window_select_ns),
        window_advance: summarize_ns(&mut window_advance_ns),
    }
}

fn summarize_cardinality(timings: &[QueryTimings]) -> CardinalitySummary {
    let mut values: Vec<u64> = timings.iter().map(|t| t.cardinality).collect();
    values.sort_unstable();
    let n = values.len().max(1);
    CardinalitySummary {
        min: *values.first().unwrap_or(&0),
        max: *values.last().unwrap_or(&0),
        p50: values[(n - 1) * 50 / 100],
        p95: values[(n - 1) * 95 / 100],
    }
}

fn build_json_report(
    set: &str,
    opts: &BenchOptions,
    seed: u64,
    pool_len: usize,
    sampling_restarts: u64,
    sample_combinations_wall_ns: Option<u64>,
    cardinality: CardinalitySummary,
    summaries: OpSummaries,
    timings: &[QueryTimings],
) -> BenchReportJson {
    let samples = opts.json_samples.then(|| {
        timings
            .iter()
            .map(|t| BenchSampleJson {
                cardinality: t.cardinality,
                start_rank: t.start_rank,
                intersect_ms: ns_to_ms(t.intersect_ns),
            })
            .collect()
    });

    BenchReportJson {
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
        roaring_only: opts.roaring_only,
        pool_len,
        multi_ids: opts.multi_ids,
        sampling_restarts,
        sample_combinations_wall_ms: sample_combinations_wall_ns.map(ns_to_ms),
        cardinality,
        ops: BenchReportOpsJson {
            sample_combinations: summaries.sample_combinations,
            intersect: summaries.intersect,
            count: summaries.count,
            first_50: summaries.first_50,
            offset_10000_50: summaries.offset_10000_50,
            window_skip: summaries.window_skip,
            window_select: summaries.window_select,
            window_advance: summaries.window_advance,
        },
        samples,
    }
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
    let mut by_type = CatalogByType {
        triggers: Vec::new(),
        conditions: Vec::new(),
        outputs: Vec::new(),
    };

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
            let idx = catalog_ids.len();
            catalog_ids.push(CatalogIdEntry {
                id_gd: e.id_gd,
                element_type: element_type.clone(),
            });
            match element_type.as_str() {
                "TRIGGER" => by_type.triggers.push(idx),
                "CONDITION" => by_type.conditions.push(idx),
                "OUTPUT" => by_type.outputs.push(idx),
                _ => {}
            }
        }
    }

    Ok(PreloadedIndex {
        per_line,
        whole_card,
        catalog_ids,
        by_type,
    })
}

fn filter_nonempty_solo_ids(index: &mut PreloadedIndex, whole_card: bool) {
    let candidates = index.catalog_ids.clone();
    index.catalog_ids = candidates
        .into_iter()
        .filter(|entry| {
            let solo = buckets_for_single_id(entry);
            !run_query(index, &solo, whole_card).is_empty()
        })
        .collect();
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
    push_id_to_bucket(&mut buckets, entry);
    buckets
}

fn push_id_to_bucket(buckets: &mut IdGdQueryBuckets, entry: &CatalogIdEntry) {
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

fn buckets_contains_id(buckets: &IdGdQueryBuckets, id_gd: u32) -> bool {
    buckets.triggers.contains(&id_gd)
        || buckets.conditions.contains(&id_gd)
        || buckets.outputs.contains(&id_gd)
}

fn buckets_picked_count(buckets: &IdGdQueryBuckets) -> usize {
    buckets.triggers.len() + buckets.conditions.len() + buckets.outputs.len()
}

fn sample_query_buckets(
    index: &PreloadedIndex,
    rng: &mut XorShift64Star,
    multi_ids: Option<(usize, usize)>,
    whole_card: bool,
) -> Result<(IdGdQueryBuckets, u64)> {
    match multi_ids {
        None => {
            let entry = &index.catalog_ids[rng.next_usize(index.catalog_ids.len())];
            Ok((buckets_for_single_id(entry), 0))
        }
        Some((min, max)) => {
            let k = min + rng.next_usize(max - min + 1);
            sample_nonempty_buckets(index, rng, k, whole_card)
        }
    }
}

fn sample_nonempty_buckets(
    index: &PreloadedIndex,
    rng: &mut XorShift64Star,
    k: usize,
    whole_card: bool,
) -> Result<(IdGdQueryBuckets, u64)> {
    let mut restarts: u64 = 0;
    for _ in 0..MAX_PACK_RETRIES {
        let anchor_idx = rng.next_usize(index.catalog_ids.len());
        let mut builder =
            PackBuilder::from_entry(index, &index.catalog_ids[anchor_idx], whole_card);

        while builder.picked_count() < k {
            let viable = builder.collect_viable_indices(index);
            if viable.is_empty() {
                restarts += 1;
                break;
            }
            let pick = viable[rng.next_usize(viable.len())];
            builder.add(index, &index.catalog_ids[pick]);
        }

        if builder.picked_count() == k && builder.query_nonempty(index) {
            return Ok((builder.buckets, restarts));
        }
        restarts += 1;
    }

    anyhow::bail!(
        "failed to sample {k} non-empty idGd ids after {MAX_PACK_RETRIES} pack attempts"
    );
}

impl PackBuilder {
    fn from_entry(index: &PreloadedIndex, entry: &CatalogIdEntry, whole_card: bool) -> Self {
        let buckets = buckets_for_single_id(entry);
        let cache = if whole_card {
            let mut trigger = RoaringBitmap::new();
            let mut condition = RoaringBitmap::new();
            let mut output = RoaringBitmap::new();
            or_id_whole(&mut trigger, &mut condition, &mut output, index, entry);
            PackCache::Whole {
                trigger,
                condition,
                output,
            }
        } else {
            let mut trigger = empty_per_line_unions();
            let mut condition = empty_per_line_unions();
            let mut output = empty_per_line_unions();
            or_id_per_line(&mut trigger, &mut condition, &mut output, index, entry);
            PackCache::PerLine {
                trigger,
                condition,
                output,
            }
        };
        Self { buckets, cache }
    }

    fn picked_count(&self) -> usize {
        buckets_picked_count(&self.buckets)
    }

    fn add(&mut self, index: &PreloadedIndex, entry: &CatalogIdEntry) {
        push_id_to_bucket(&mut self.buckets, entry);
        match &mut self.cache {
            PackCache::PerLine {
                trigger,
                condition,
                output,
            } => or_id_per_line(trigger, condition, output, index, entry),
            PackCache::Whole {
                trigger,
                condition,
                output,
            } => or_id_whole(trigger, condition, output, index, entry),
        }
    }

    /// Same outcome as `!run_query(...).is_empty()` for the current buckets.
    fn query_nonempty(&self, index: &PreloadedIndex) -> bool {
        match &self.cache {
            PackCache::PerLine {
                trigger,
                condition,
                output,
            } => EffectLine::ALL.iter().any(|&line| {
                !line_intersect_on_line(
                    index,
                    &self.buckets,
                    trigger,
                    condition,
                    output,
                    line,
                    None,
                )
                .is_empty()
            }),
            PackCache::Whole {
                trigger,
                condition,
                output,
            } => {
                !whole_card_intersect(index, &self.buckets, trigger, condition, output, None)
                    .is_empty()
            }
        }
    }

    /// Scan catalog by element type (precomputed indices) with fast same-line viability.
    fn collect_viable_indices(&self, index: &PreloadedIndex) -> Vec<usize> {
        let mut viable = Vec::new();
        for idx in index
            .by_type
            .triggers
            .iter()
            .chain(index.by_type.conditions.iter())
            .chain(index.by_type.outputs.iter())
        {
            let entry = &index.catalog_ids[*idx];
            if buckets_contains_id(&self.buckets, entry.id_gd) {
                continue;
            }
            if self.candidate_viable(index, entry) {
                viable.push(*idx);
            }
        }
        viable
    }

    fn candidate_viable(&self, index: &PreloadedIndex, entry: &CatalogIdEntry) -> bool {
        match &self.cache {
            PackCache::PerLine {
                trigger,
                condition,
                output,
            } => EffectLine::ALL.iter().any(|&line| {
                !line_intersect_on_line(
                    index,
                    &self.buckets,
                    trigger,
                    condition,
                    output,
                    line,
                    Some(entry),
                )
                .is_empty()
            }),
            PackCache::Whole {
                trigger,
                condition,
                output,
            } => {
                !whole_card_intersect(index, &self.buckets, trigger, condition, output, Some(entry))
                    .is_empty()
            }
        }
    }
}

fn empty_per_line_unions() -> BTreeMap<EffectLine, RoaringBitmap> {
    EffectLine::ALL
        .into_iter()
        .map(|line| (line, RoaringBitmap::new()))
        .collect()
}

fn or_id_per_line(
    trigger: &mut BTreeMap<EffectLine, RoaringBitmap>,
    condition: &mut BTreeMap<EffectLine, RoaringBitmap>,
    output: &mut BTreeMap<EffectLine, RoaringBitmap>,
    index: &PreloadedIndex,
    entry: &CatalogIdEntry,
) {
    let target = match entry.element_type.as_str() {
        "TRIGGER" => trigger,
        "CONDITION" => condition,
        "OUTPUT" => output,
        _ => return,
    };
    for line in EffectLine::ALL {
        if let Some(bm) = index.per_line.get(&(entry.id_gd, line)) {
            *target.entry(line).or_default() |= bm;
        }
    }
}

fn or_id_whole(
    trigger: &mut RoaringBitmap,
    condition: &mut RoaringBitmap,
    output: &mut RoaringBitmap,
    index: &PreloadedIndex,
    entry: &CatalogIdEntry,
) {
    let Some(bm) = index.whole_card.get(&entry.id_gd) else {
        return;
    };
    match entry.element_type.as_str() {
        "TRIGGER" => *trigger |= bm,
        "CONDITION" => *condition |= bm,
        "OUTPUT" => *output |= bm,
        _ => {}
    }
}

fn line_intersect_on_line(
    index: &PreloadedIndex,
    buckets: &IdGdQueryBuckets,
    trigger: &BTreeMap<EffectLine, RoaringBitmap>,
    condition: &BTreeMap<EffectLine, RoaringBitmap>,
    output: &BTreeMap<EffectLine, RoaringBitmap>,
    line: EffectLine,
    extra: Option<&CatalogIdEntry>,
) -> RoaringBitmap {
    let mut groups: Vec<RoaringBitmap> = Vec::new();

    if !buckets.triggers.is_empty() || extra.is_some_and(|e| e.element_type == "TRIGGER") {
        let mut u = trigger.get(&line).cloned().unwrap_or_default();
        if let Some(e) = extra.filter(|e| e.element_type == "TRIGGER") {
            if let Some(bm) = index.per_line.get(&(e.id_gd, line)) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    if !buckets.conditions.is_empty() || extra.is_some_and(|e| e.element_type == "CONDITION") {
        let mut u = condition.get(&line).cloned().unwrap_or_default();
        if let Some(e) = extra.filter(|e| e.element_type == "CONDITION") {
            if let Some(bm) = index.per_line.get(&(e.id_gd, line)) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    if !buckets.outputs.is_empty() || extra.is_some_and(|e| e.element_type == "OUTPUT") {
        let mut u = output.get(&line).cloned().unwrap_or_default();
        if let Some(e) = extra.filter(|e| e.element_type == "OUTPUT") {
            if let Some(bm) = index.per_line.get(&(e.id_gd, line)) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    intersect_bitmaps(groups)
}

fn whole_card_intersect(
    index: &PreloadedIndex,
    buckets: &IdGdQueryBuckets,
    trigger: &RoaringBitmap,
    condition: &RoaringBitmap,
    output: &RoaringBitmap,
    extra: Option<&CatalogIdEntry>,
) -> RoaringBitmap {
    let mut groups: Vec<RoaringBitmap> = Vec::new();

    if !buckets.triggers.is_empty() || extra.is_some_and(|e| e.element_type == "TRIGGER") {
        let mut u = trigger.clone();
        if let Some(e) = extra.filter(|e| e.element_type == "TRIGGER") {
            if let Some(bm) = index.whole_card.get(&e.id_gd) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    if !buckets.conditions.is_empty() || extra.is_some_and(|e| e.element_type == "CONDITION") {
        let mut u = condition.clone();
        if let Some(e) = extra.filter(|e| e.element_type == "CONDITION") {
            if let Some(bm) = index.whole_card.get(&e.id_gd) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    if !buckets.outputs.is_empty() || extra.is_some_and(|e| e.element_type == "OUTPUT") {
        let mut u = output.clone();
        if let Some(e) = extra.filter(|e| e.element_type == "OUTPUT") {
            if let Some(bm) = index.whole_card.get(&e.id_gd) {
                u |= bm;
            }
        }
        groups.push(u);
    }

    intersect_bitmaps(groups)
}

fn intersect_bitmaps(groups: Vec<RoaringBitmap>) -> RoaringBitmap {
    let mut it = groups.into_iter();
    let Some(mut acc) = it.next() else {
        return RoaringBitmap::new();
    };
    for g in it {
        acc &= g;
    }
    acc
}

fn window_start_rank(cardinality: u64) -> u64 {
    if cardinality >= 10_000 {
        10_000
    } else {
        cardinality.saturating_sub(50)
    }
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

fn op_list_first_50(
    catalog: &Catalog,
    cards_data: &[u8],
    bitmap: &RoaringBitmap,
) -> Result<Vec<DecodedCardObject>> {
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
    let start = window_start_rank(bitmap.len());
    let mut out = Vec::with_capacity(50);
    for bit in bitmap.iter().skip(start as usize).take(50) {
        if let Some(obj) = decode_card_object(catalog, cards_data, bit)? {
            out.push(obj);
        }
    }
    Ok(out)
}

fn op_window_skip_take(bitmap: &RoaringBitmap, start: u64) -> Vec<u32> {
    bitmap.iter().skip(start as usize).take(50).collect()
}

fn op_window_select_loop(bitmap: &RoaringBitmap, start: u64) -> Vec<u32> {
    (0..50)
        .filter_map(|i| bitmap.select((start + i) as u32))
        .collect()
}

fn op_window_select_advance(bitmap: &RoaringBitmap, start: u64) -> Vec<u32> {
    let Some(first) = bitmap.select(start as u32) else {
        return Vec::new();
    };
    let mut it = bitmap.iter();
    it.advance_to(first);
    it.take(50).collect()
}

fn decode_card_object(
    catalog: &Catalog,
    cards_data: &[u8],
    bit: u32,
) -> Result<Option<DecodedCardObject>> {
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
    sampling_restarts: u64,
    sample_combinations_wall_ns: Option<u64>,
    cardinality: &CardinalitySummary,
    summaries: &OpSummaries,
) {
    let index_mode = if opts.whole_card {
        "whole-card"
    } else {
        "per-line"
    };
    let roaring_note = if opts.roaring_only { " roaring_only" } else { "" };
    if let Some((min, max)) = opts.multi_ids {
        println!(
            "bench-query: set={set} seed={seed} index={index_mode} mode=multi multi_ids={min}-{max}{roaring_note} warmup={} queries={}",
            opts.warmup, opts.queries
        );
    } else {
        println!(
            "bench-query: set={set} seed={seed} index={index_mode} mode=single{roaring_note} warmup={} queries={} pool_len={pool_len}",
            opts.warmup, opts.queries
        );
    }
    if let Some(wall_ns) = sample_combinations_wall_ns {
        println!(
            "multi_id_combinations: {} packs in {:.3}s ({} pack restarts)",
            summaries
                .sample_combinations
                .as_ref()
                .map(|s| s.count)
                .unwrap_or(0),
            ns_to_ms(wall_ns),
            sampling_restarts
        );
    }
    println!(
        "result_cardinality: min={} p50={} p95={} max={}",
        cardinality.min, cardinality.p50, cardinality.p95, cardinality.max
    );
    println!();
    if let Some(s) = &summaries.sample_combinations {
        print_op("sample_combinations", s);
    }
    print_op("intersect", &summaries.intersect);
    print_op("count", &summaries.count);
    if let Some(s) = &summaries.first_50 {
        print_op("first_50", s);
    }
    if let Some(s) = &summaries.offset_10000_50 {
        print_op("offset_10000_50", s);
    }
    print_op("window_skip", &summaries.window_skip);
    print_op("window_select", &summaries.window_select);
    print_op("window_advance", &summaries.window_advance);
}

fn print_op(name: &str, s: &StatsSummary) {
    fn fmt_ms(v: f64) -> String {
        format!("{v:.3}ms")
    }

    println!(
        "{name:<18} n={:<6} min={:<12} p50={:<12} p95={:<12} mean={:<12} max={:<12}",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_index() -> PreloadedIndex {
        let mut per_line = BTreeMap::new();
        per_line.insert((1, EffectLine::M1), RoaringBitmap::from_iter([10, 11]));
        per_line.insert((2, EffectLine::M1), RoaringBitmap::from_iter([11, 12]));
        per_line.insert((3, EffectLine::M2), RoaringBitmap::from_iter([20]));
        let catalog_ids = vec![
            CatalogIdEntry {
                id_gd: 1,
                element_type: "TRIGGER".to_string(),
            },
            CatalogIdEntry {
                id_gd: 2,
                element_type: "CONDITION".to_string(),
            },
            CatalogIdEntry {
                id_gd: 3,
                element_type: "OUTPUT".to_string(),
            },
        ];
        let by_type = CatalogByType {
            triggers: vec![0],
            conditions: vec![1],
            outputs: vec![2],
        };
        PreloadedIndex {
            per_line,
            whole_card: BTreeMap::new(),
            catalog_ids,
            by_type,
        }
    }

    #[test]
    fn fast_viability_matches_run_query() {
        let index = tiny_index();
        let anchor = &index.catalog_ids[0];
        let builder = PackBuilder::from_entry(&index, anchor, false);
        for (i, entry) in index.catalog_ids.iter().enumerate() {
            if buckets_contains_id(&builder.buckets, entry.id_gd) {
                continue;
            }
            let fast = builder.candidate_viable(&index, entry);
            let mut tentative = builder.buckets.clone();
            push_id_to_bucket(&mut tentative, entry);
            let slow = !run_query(&index, &tentative, false).is_empty();
            assert_eq!(fast, slow, "mismatch for catalog index {i}");
        }
    }
}
