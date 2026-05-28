use crate::build;
use crate::bench_query;
use crate::decode;
use crate::merge;
use crate::query;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn parse_multi_ids_range(s: &str) -> Result<(usize, usize), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err("expected MIN-MAX (e.g. 6-12)".to_string());
    }
    let min: usize = parts[0]
        .trim()
        .parse()
        .map_err(|_| "MIN must be a positive integer".to_string())?;
    let max: usize = parts[1]
        .trim()
        .parse()
        .map_err(|_| "MAX must be a positive integer".to_string())?;
    if min == 0 || max == 0 {
        return Err("MIN and MAX must be >= 1".to_string());
    }
    if min > max {
        return Err("MIN must be <= MAX".to_string());
    }
    Ok((min, max))
}

#[derive(Parser)]
#[command(name = "alt-indexer", about = "Index card JSON by idGd into Roaring bitmaps")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Crawl a dataset and write catalog + idGd bitmaps.
    Build {
        /// Dataset root containing `json/<SET>/...`
        #[arg(long)]
        root: PathBuf,
        /// Set code (e.g. COREKS, ALIZE, BISE)
        #[arg(long)]
        set: String,
        /// Output directory (writes `<out>/<SET>/...`)
        #[arg(long)]
        out: PathBuf,
        /// Stop discovery and indexing after this many files (for testing).
        #[arg(long)]
        limit: Option<usize>,
        /// Print build phase timings (read, parse, process, write). Also enabled by ALT_INDEXER_PROFILE=1.
        #[arg(long)]
        profile: bool,
    },
    /// Decode a global bit index to a card reference.
    Decode {
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        bit: u32,
    },
    /// Query how many cards contain an idGd.
    Query {
        #[arg(long)]
        index_dir: PathBuf,
        #[arg(long)]
        set: String,
        /// Comma-separated list of idGd values (e.g. `--id-gd 24,191,76`).
        #[arg(long, value_delimiter = ',')]
        id_gd: Vec<u32>,
        /// Decode and print up to N matching card references.
        #[arg(long)]
        list: Option<usize>,
        /// Show translated effect text instead of a table.
        #[arg(long, default_value_t = false)]
        show_effect: bool,
        /// Locale key for effect translation (e.g. en_US, fr_FR).
        #[arg(long, default_value = "en_US")]
        locale: String,
    },
    /// Merge multiple existing per-SET indexes into one merged index.
    Merge {
        /// Directory containing per-SET folders, e.g. `<index-dir>/<SET>/catalog.json`.
        #[arg(long)]
        index_dir: PathBuf,
        /// Comma-separated list of SET codes in precedence order (used for overlap grouping and tie-breaking).
        #[arg(long)]
        sets: String,
        /// Full output directory for merged index (files written directly under this folder).
        #[arg(long)]
        out: PathBuf,
    },
    /// Benchmark random idGd queries against an existing index (preloads bitmaps + cards.bin).
    BenchQuery {
        #[arg(long)]
        index_dir: PathBuf,
        #[arg(long)]
        set: String,
        /// Number of timed queries to run.
        #[arg(long, default_value_t = 5000)]
        queries: usize,
        /// Simulate multi-id queries by picking K ids per query (K is random in MIN-MAX),
        /// then doing (TRIGGER union) ∩ (CONDITION union) ∩ (OUTPUT union) (skipping empty groups).
        /// Example: `--multi-ids 6-12`.
        #[arg(long, value_parser = parse_multi_ids_range)]
        multi_ids: Option<(usize, usize)>,
        /// RNG seed (deterministic). If omitted, a default constant is used.
        #[arg(long)]
        seed: Option<u64>,
        /// Warmup iterations (executed but not recorded).
        #[arg(long, default_value_t = 200)]
        warmup: usize,
        /// Optional machine-readable JSON output path.
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Print first N sampled queries (sanity check; adds output noise).
        #[arg(long)]
        print_samples: Option<usize>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build {
            root,
            set,
            out,
            limit,
            profile,
        } => {
            let summary = build::build(
                &root,
                &set,
                &out,
                build::BuildOptions {
                    file_limit: limit,
                    profile,
                },
            )?;
            let limit_note = match summary.file_limit {
                Some(n) if summary.stopped_early => format!(" (limit {n})"),
                Some(n) => format!(" (under limit {n})"),
                None => String::new(),
            };
            println!(
                "built {}: {} files{}, {} families, {} idGd bitmaps, bit span {}",
                summary.output_dir.display(),
                summary.files_processed,
                limit_note,
                summary.catalog.families.len(),
                summary.id_gd_count,
                summary.catalog.total_bit_span
            );
        }
        Command::Decode { catalog, bit } => {
            let decoded = decode::decode_bit(&catalog, bit)?;
            println!("{}", decoded.reference);
            println!(
                "familyId={} uniqueID={}",
                decoded.family_id, decoded.unique_id
            );
        }
        Command::Query {
            index_dir,
            set,
            id_gd,
            list,
            show_effect,
            locale,
        } => {
            if show_effect {
                let result =
                    query::query_id_gds_effect_text(&index_dir, &set, &id_gd, list, &locale)?;
                println!("query: {} cards", result.cardinality);
                if !result.recap_lines.is_empty() {
                    println!();
                    for line in &result.recap_lines {
                        println!("{line}");
                    }
                }
                if !result.cards.is_empty() {
                    println!();
                    for card in &result.cards {
                        println!("{}", card.reference);
                        println!(
                            "Cost: {} / {}          Power: O:{} / M:{} / F:{}",
                            card.hand, card.reserve, card.o, card.m, card.f
                        );
                        for line in &card.effect_lines {
                            println!("{line}");
                        }
                        println!("-----------------");
                    }
                }
            } else {
                let result = query::query_id_gds(&index_dir, &set, &id_gd, list)?;
                println!("query: {} cards", result.cardinality);
                if !result.rows.is_empty() {
                    println!();
                    println!(
                        "{:<7}  {:<28}  {:>3} {:>3} {:>2} {:>2} {:>2}  {:<40}  {:<16}",
                        "index", "reference", "Hc", "Rc", "M", "O", "F", "main effect", "echo effect"
                    );
                    println!("{}", "-".repeat(7 + 2 + 28 + 2 + 3 + 1 + 3 + 1 + 2 + 1 + 2 + 1 + 2 + 2 + 40 + 2 + 16));
                    for row in &result.rows {
                        println!(
                            "{:<7}  {:<28}  {:>3} {:>3} {:>2} {:>2} {:>2}  {:<40}  {:<16}",
                            row.card_index,
                            row.reference,
                            row.hand,
                            row.reserve,
                            row.m,
                            row.o,
                            row.f,
                            row.main_effect,
                            if row.echo_effect.is_empty() {
                                "<none>"
                            } else {
                                &row.echo_effect
                            }
                        );
                    }
                }
            }
        }
        Command::Merge {
            index_dir,
            sets,
            out,
        } => {
            let summary = merge::merge_indexes(&index_dir, &sets, &out)?;
            println!(
                "merged {}: {} source sets, {} cards, {} families, {} idGd bitmaps, bit span {}",
                summary.output_dir.display(),
                summary.source_sets.len(),
                summary.card_count,
                summary.family_count,
                summary.id_gd_count,
                summary.total_bit_span
            );
        }
        Command::BenchQuery {
            index_dir,
            set,
            queries,
            multi_ids,
            seed,
            warmup,
            json_out,
            print_samples,
        } => {
            bench_query::run(
                &index_dir,
                &set,
                bench_query::BenchOptions {
                    queries,
                    seed,
                    warmup,
                    multi_ids,
                    json_out,
                    print_samples,
                },
            )?;
        }
    }
    Ok(())
}
