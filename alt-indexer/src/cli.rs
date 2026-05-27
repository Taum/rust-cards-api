use crate::build;
use crate::decode;
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
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { root, set, out, limit } => {
            let summary = build::build(
                &root,
                &set,
                &out,
                build::BuildOptions { file_limit: limit },
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
        } => {
            let result = query::query_id_gd(&index_dir, &set, id_gd, list)?;
            println!("idGd {}: {} cards", result.id_gd, result.cardinality);
            for reference in &result.references {
                println!("  {reference}");
            }
        }
    }
    Ok(())
}
