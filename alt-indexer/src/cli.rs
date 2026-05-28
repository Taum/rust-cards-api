use crate::build;
use crate::decode;
use crate::merge;
use crate::query;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        #[arg(long)]
        id_gd: u32,
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
                let result = query::query_id_gd_effect_text(&index_dir, &set, id_gd, list, &locale)?;
                println!("idGd {}: {} cards", result.id_gd, result.cardinality);
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
                let result = query::query_id_gd(&index_dir, &set, id_gd, list)?;
                println!("idGd {}: {} cards", result.id_gd, result.cardinality);
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
    }
    Ok(())
}
