use crate::analyze::{self, AnalyzeOptions};
use crate::download::{self, DownloadOptions};
use crate::locale::LocalePolicy;
use crate::resolve::{self, ResolveOptions};
use crate::sample::{self, ComboMode, SampleOptions};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "image-sampler",
    about = "Sample, resolve, and download unique-card images from the merged index"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Count ability combinations from `cards.bin` + `catalog.json`. No JSON crawl.
    Analyze {
        #[arg(long, default_value = "build/full_index")]
        index_dir: PathBuf,
        #[arg(long, default_value = "ALL_SETS")]
        set: String,
        #[arg(long)]
        out_json: Option<PathBuf>,
    },
    /// Build a sampling plan: shape floor + unique strict tuples + tiered locales (en always).
    Sample {
        #[arg(long, default_value = "build/full_index")]
        index_dir: PathBuf,
        #[arg(long, default_value = "ALL_SETS")]
        set: String,
        /// English card budget (unique strict tuples). Default 200_000.
        #[arg(long, conflicts_with = "budget_fraction")]
        budget: Option<usize>,
        #[arg(long, conflicts_with = "budget")]
        budget_fraction: Option<f64>,
        /// Root with `cards-unique-<SET>/json/...` (required to detect available locales).
        #[arg(long)]
        equinox_root: PathBuf,
        /// Fraction of cards that receive all five locales (default 1%).
        #[arg(long, default_value_t = 0.01)]
        full_locale_fraction: f64,
        /// Fraction of cards that receive English + French, including full tier (default 10%).
        #[arg(long, default_value_t = 0.10)]
        fr_locale_fraction: f64,
        #[arg(long, default_value = "set")]
        combo_mode: String,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value = "out/plan.jsonl")]
        out: PathBuf,
        #[arg(long, default_value = "out/plan-summary.json")]
        out_summary: PathBuf,
    },
    /// Open sampled cards' JSONs; emit one row per card with per-locale URLs.
    ResolveUrls {
        #[arg(long, default_value = "out/plan.jsonl")]
        plan: PathBuf,
        #[arg(long)]
        equinox_root: PathBuf,
        #[arg(long, default_value = "out/plan-resolved.jsonl")]
        out: PathBuf,
        #[arg(long, default_value = "out/resolve-errors.jsonl")]
        out_errors: PathBuf,
    },
    /// Fetch resolved locale URLs and store images under `out/images/`.
    Download {
        #[arg(long, default_value = "out/plan-resolved.jsonl")]
        plan: PathBuf,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
        #[arg(long, default_value_t = 3)]
        max_retries: u32,
        #[arg(long, default_value_t = 750)]
        backoff_ms: u64,
        #[arg(long, default_value_t = 30)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 5)]
        spot_check_n: usize,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value_t = false)]
        no_proxy: bool,
        /// Proxy `&w=` when rebuilding URLs from `rel_path` (default 1200).
        #[arg(long, default_value_t = 1200)]
        width: u32,
        /// Proxy `&q=` when rebuilding URLs from `rel_path` (default 75).
        #[arg(long, default_value_t = 75)]
        quality: u32,
        #[arg(long, default_value = "image-sampler/0.1 (+rust-cards-api)")]
        user_agent: String,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Max HTTP image fetches per second across all workers (`0` = unlimited).
        #[arg(long, default_value_t = 2.0)]
        images_per_second: f64,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze {
            index_dir,
            set,
            out_json,
        } => {
            let opts = AnalyzeOptions {
                index_dir,
                set,
                out_json,
            };
            let report = analyze::run(&opts)?;
            analyze::print_report(&report);
        }
        Command::Sample {
            index_dir,
            set,
            budget,
            budget_fraction,
            equinox_root,
            full_locale_fraction,
            fr_locale_fraction,
            combo_mode,
            seed,
            out,
            out_summary,
        } => {
            let combo_mode = ComboMode::parse(&combo_mode)?;
            let resolved_budget =
                resolve_budget(&index_dir, &set, budget, budget_fraction)?;
            let locale_policy = LocalePolicy {
                full_fraction: full_locale_fraction,
                fr_fraction: fr_locale_fraction,
            };
            locale_policy.validate()?;
            let opts = SampleOptions {
                index_dir,
                set,
                budget: resolved_budget,
                locale_policy,
                combo_mode,
                seed,
                equinox_root,
                out_plan: out,
                out_summary,
            };
            let summary = sample::run(&opts)?;
            sample::print_summary(&summary);
        }
        Command::ResolveUrls {
            plan,
            equinox_root,
            out,
            out_errors,
        } => {
            let opts = ResolveOptions {
                plan,
                equinox_root,
                out_resolved: out,
                out_errors,
            };
            let summary = resolve::run(&opts)?;
            resolve::print_summary(&summary);
        }
        Command::Download {
            plan,
            out_dir,
            concurrency,
            max_retries,
            backoff_ms,
            timeout_secs,
            spot_check_n,
            force,
            no_proxy,
            width,
            quality,
            user_agent,
            seed,
            images_per_second,
        } => {
            let opts = DownloadOptions {
                plan_resolved: plan,
                out_dir,
                concurrency,
                max_retries,
                backoff_ms,
                timeout_secs,
                spot_check_n,
                force,
                use_proxy: !no_proxy,
                proxy_width: width,
                proxy_quality: quality,
                user_agent,
                seed,
                images_per_second,
            };
            let summary = download::run(&opts)?;
            download::print_summary(&summary);
        }
    }
    Ok(())
}

fn resolve_budget(
    index_dir: &std::path::Path,
    set: &str,
    explicit: Option<usize>,
    fraction: Option<f64>,
) -> Result<usize> {
    if let Some(b) = explicit {
        return Ok(b);
    }
    let shape_minimum = sample::shape_coverage_minimum(index_dir, set)?;
    if let Some(f) = fraction {
        anyhow::ensure!(f > 0.0 && f <= 1.0, "budget-fraction must be in (0, 1]");
        let valid = count_valid_cards(index_dir, set)?;
        let budget = ((valid as f64) * f).ceil() as usize;
        anyhow::ensure!(budget > 0, "computed budget is 0; raise --budget-fraction");
        anyhow::ensure!(
            budget >= shape_minimum,
            "budget-fraction yields {budget} cards, below shape-coverage minimum {shape_minimum}"
        );
        return Ok(budget);
    }
    Ok(200_000)
}

fn count_valid_cards(index_dir: &std::path::Path, set: &str) -> Result<u64> {
    use index_core::catalog::Catalog;
    use index_core::compact::{CompactCardView, RECORD_SIZE};
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let cards = std::fs::read(set_dir.join("cards.bin"))?;
    let expected = catalog.total_bit_span as usize * RECORD_SIZE;
    anyhow::ensure!(cards.len() >= expected, "cards.bin truncated");
    let mut valid: u64 = 0;
    for family in &catalog.families {
        let start = family.start_bit;
        let end = family.start_bit + family.max_unique_id;
        for ci in start..end {
            if let Some(v) = CompactCardView::from_data(&cards, ci) {
                if v.faction_code() != 0 {
                    valid += 1;
                }
            }
        }
    }
    Ok(valid)
}
