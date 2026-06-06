# AGENTS.md

Project-wide notes for AI agents working in this repo.

## Cargo workspace

The repository root [`Cargo.toml`](Cargo.toml) is a workspace with members `alt-indexer` and `uniques-http-api`. Run `cargo build`, `cargo test`, and `cargo run -p <crate>` from the repo root. Build output goes to `target/` at the root (not under individual crate directories).

## Index layout (`alt-indexer/full_index/`)

This directory is gitignored (not committed) but present on disk and readable.

- **`alt-indexer/full_index/ALL_SETS`** is the index to use. It is the **merged** index
  (`manifest.json` → `"kind": "merge"`) combining all single-set indexes into one
  global bit space. This is what `uniques-http-api` loads by default
  (`INDEX_PATH=./alt-indexer/full_index/ALL_SETS`, see `uniques-http-api/.env.local`).
- The sibling folders (`CORE`, `COREKS`, `ALIZE`, `BISE`, `CYCLONE`, `DUSTER`, `EOLE`)
  are **single-set** indexes produced by `alt-indexer build`. The `merge` subcommand
  combines them into `ALL_SETS`.

Current `ALL_SETS` stats (from `manifest.json`): ~5,455,928 cards (`total_bit_span`),
527 families, 818 distinct `idGd` effect parts.

## Note on `card_count` (the ~5.4M number)

`card_count` / `total_bit_span` counts **unique-card bit slots** (per-UniqueID within each
family, including padding for gaps), not distinct printable cards. The distinct effect-part
space is small: `id_gd_count` = 818.

## Reading gitignored files

`Glob` and the codebase search tools respect `.gitignore`, so they will not surface files
under `full_index/`. Use `Read` with an explicit path, or PowerShell `Get-ChildItem` /
`Get-Content`, to inspect the index. (Shell here is PowerShell on Windows — use PS syntax,
not `2>/dev/null`.)
