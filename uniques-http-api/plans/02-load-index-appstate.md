# Plan 02: Load full index into AppState

## Goal

Eagerly load the merged `ALL_SETS` index directory into memory at startup and share it across all HTTP requests via `Arc<AppState>` (read-only, no per-request reload).

## Index layout

See [`docs/ALL_SETS-index-format.md`](../../docs/ALL_SETS-index-format.md). Under `INDEX_PATH` (the index folder itself, e.g. `.../ALL_SETS`):

| Asset | Loaded into |
| ----- | ----------- |
| `catalog.json` | `Catalog` |
| `manifest.json` | `IndexManifest` |
| `idgd_catalog.json` | `IdGdCatalog` |
| `stats_summary.json` | `StatsSummary` |
| `factions_summary.json` | `FactionsSummary` |
| `cards.bin` | `Vec<u8>` (validated: `total_bit_span * 32`) |
| `id_gd/*.roar` | `id_gd_whole` + `id_gd_per_line` (from catalog metadata) |
| `stats/**.roar` | `stats` buckets per `StatField` |
| `factions/*.roar` | `factions` per `Faction` |

## AppState

- [`src/state.rs`](../src/state.rs): `AppState` wraps `Arc<AppStateInner>` for cheap clone per handler.
- [`src/loader.rs`](../src/loader.rs): `load_index(&Path) -> Result<AppState>` walks summaries and loads every referenced `.roar` file.
- Reuses types from [`alt-indexer`](../../alt-indexer) (`Catalog`, `IdGdCatalog`, `EffectLine`, `StatField`, `Faction`, `BitmapStore::load`).

## Startup

- `INDEX_PATH` from `.env`, overridable via `.env.local` (`load_env`: `dotenvy::from_path` then `from_path_override`, paths relative to the crate directory).
- `main` calls `load_index` before binding `0.0.0.0:8234`.
- Axum router: `.with_state(Arc<AppState>)`.

## Deferred (later plans)

- HTTP routes to query cards / idGd / stats.
- mmap for `cards.bin` if memory becomes an issue.
- Optional lazy loading or subset indexes.

## Success criteria

- [x] `cargo test` loads `tests/fixtures/minimal_index` and decodes a reference.
- [x] Server refuses to start without `INDEX_PATH` (clear error).
- [ ] Manual run against a real `ALL_SETS` index (operator-provided path).
