# AGENTS.md

Project-wide notes for AI agents working in this repo.

## Cargo workspace

The repository root [`Cargo.toml`](Cargo.toml) is a workspace with members `index-core`, `cli-indexer`, and `uniques-http-api`. Run `cargo build`, `cargo test`, and `cargo run -p <crate>` from the repo root. Build output goes to `target/` at the root (not under individual crate directories).

- **`index-core`**: shared index library (build, merge, load types, query, bitmaps, catalogs).
- **`cli-indexer`**: CLI binary (`build`, `merge`, `query`, etc.) — thin wrapper over `index-core`.
- **`uniques-http-api`**: HTTP server; depends on `index-core` for index types and query helpers.

## Index layout (`build/`)

Both directories below are gitignored (not committed) but present on disk and readable.

- **`build/full_index/ALL_SETS`** is the merged index to use (`manifest.json` → `"kind": "merge"`).
  This is what `uniques-http-api` loads by default
  (`INDEX_PATH=./build/full_index/ALL_SETS`, see `uniques-http-api/.env.local`).
- **`build/sets_index/`** holds per-set indexes (`CORE`, `COREKS`, `ALIZE`, `BISE`, `CYCLONE`, `DUSTER`, `EOLE`, …)
  produced by `cli-indexer build`. The `merge` subcommand combines them into `build/full_index/ALL_SETS`.

Current `ALL_SETS` stats (from `manifest.json`): ~5,455,928 cards (`total_bit_span`),
527 families, 818 distinct `idGd` effect parts.

## Note on `card_count` (the ~5.4M number)

`card_count` / `total_bit_span` counts **unique-card bit slots** (per-UniqueID within each
family, including padding for gaps), not distinct printable cards. The distinct effect-part
space is small: `id_gd_count` = 818.

## Reading gitignored files

`Glob` and the codebase search tools respect `.gitignore`, so they will not surface files
under `build/full_index/` or `build/sets_index/`. Use `Read` with an explicit path, or PowerShell `Get-ChildItem` /
`Get-Content`, to inspect the index. (Shell here is PowerShell on Windows — use PS syntax,
not `2>/dev/null`.)
