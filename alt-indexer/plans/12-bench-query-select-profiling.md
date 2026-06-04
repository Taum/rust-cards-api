# Bench-query: intersection timing, non-empty sampling, select profiling

## Goal

Extend `bench-query` to:

1. Time **query intersection** (`execute_idgd_query_preloaded`) as the primary op.
2. Guarantee **non-empty** random idGd queries (same-line viability, mirroring `/api/v2/effects/filtered`).
3. Compare Roaring **window** strategies: `iter().skip`, `select` loop, `select` + `advance_to`.
4. Optional `--roaring-only` to skip card decode list ops.

**Out of scope**: `--profile-select` rank sweep on raw pool bitmaps.

## Timed ops (order)

| Order | Op | Notes |
|-------|-----|--------|
| 0 | `sample_combinations` | Multi-id only: non-empty pack generation (progress bar on stderr) |
| 1 | `intersect` | Preloaded unions + `&` |
| 2 | `count` | `bitmap.len()` |
| 3 | `first_50` | Unless `--roaring-only` |
| 4 | `offset_10000_50` | Unless `--roaring-only` |
| 5 | `window_skip` | 50 values from `iter().skip(start)` |
| 6 | `window_select` | 50× `bitmap.select(start + i)` |
| 7 | `window_advance` | `select(start)` + `advance_to` + `take(50)` |

`start = 10_000` if `cardinality >= 10_000`, else `cardinality - 50`.

## Non-empty sampling

- **Preload filter**: drop catalog ids whose solo query is empty.
- **Multi-id**: incremental pack with **cached per-line (or whole-card) bucket unions** and fast viability (`∃` line with non-empty same-line intersect, same intent as `/effects/filtered`). Catalog indices grouped by T/C/O. Restart pack if stuck (max 64 retries). Default `--warmup` is `5`.

## CLI

- `--roaring-only`: skip `first_50` / `offset_10000_50`; do not read `cards.bin`.
- `--json-samples`: include per-query cardinality in JSON (optional raw samples list).

## How to run

```bash
cargo build --release -p alt-indexer
./alt-indexer/target/release/alt-indexer bench-query \
  --index-dir ./alt-indexer/full_index \
  --set ALL_SETS \
  --queries 10000 \
  --multi-ids 6-12 \
  --seed 42 \
  --json-out ./bench.json
```

## Follow-up

- Shared `is_disjoint` viability helper in `alt-indexer::query` if needed by HTTP + bench.
- Apply winning window strategy to `page_cards_v2` after numbers justify it.
