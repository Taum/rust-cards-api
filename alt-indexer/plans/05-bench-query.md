# Query benchmark tool

See also [12-bench-query-select-profiling.md](./12-bench-query-select-profiling.md) for intersection timing, non-empty sampling, and window/select ops.

## What we’re benchmarking (per query)

- **Intersect**: `execute_idgd_query_preloaded` (union within T/C/O buckets, intersect groups; per-line OR across lines). Bucket sampling is **not** timed.
- **Count**: `bitmap.len()`.
- **First_50**: iterate bitmap values, take 50, and for each bit build a **full card object** by:
  - decoding reference via `Catalog::decode_bit`
  - reading the compact record from `cards.bin` via `CompactCardView::from_data(&cards_data, bit)`
  - extracting costs/power + effect triplets (3 main groups + echo)
  - extracting effect ids as idGd tuples
- **Offset_10000_50**: same as above, but the window is:
  - if `cardinality >= 10000`, skip 10000 and take 50
  - else start at `max(0, cardinality-50)` and take up to 50
  - and for each bit build the same **full card object** (not just references)
- **Window_skip / window_select / window_advance**: fetch 50 indices from the result bitmap at rank `10_000` (or last 50) without card decode — compare `iter().skip`, `select` loop, and `select` + `advance_to`.

Multi-id sampling only picks id packs whose intersection is **non-empty** (incremental viability, same intent as `/api/v2/effects/filtered`).

All bitmaps are **preloaded in memory** (no per-query disk I/O timing), per your choice.

## Integration points in this repo

- CLI entrypoint is `[src/cli.rs](./src/cli.rs)` and currently supports `build`, `decode`, `query`, `merge`.
- Query decoding helpers already exist:
  - `Catalog::load` and `Catalog::decode_bit` in `[src/catalog.rs](./src/catalog.rs)`
  - Bitmap loading via `BitmapStore::load` in `[src/bitmap.rs](./src/bitmap.rs)`
  - The list-style query currently decodes references in `query::query_id_gd` in `[src/query.rs](./src/query.rs)`
- Compact card decoding from `cards.bin` lives in `[src/compact.rs](./src/compact.rs)`: `CompactCardView::from_data` + accessors like `main_cost()`, `main_effect_group(g)`, `echo_effect()`.
- Available idGd universe is in `idgd_catalog.json` (struct in `[src/idgd_catalog.rs](./src/idgd_catalog.rs)`), which provides `entries[].id_gd` and `bitmap_file`.

## CLI design

Add a new subcommand (name bikeshed: `bench-query`) to `[src/cli.rs](./src/cli.rs)`:

- `--index-dir <PATH>`
- `--set <SET_OR_MERGED_FOLDER>`
- `--queries <N>`: number of timed queries to run
- `--multi-ids <MIN-MAX>` (optional): simulate multi-id queries by picking \(K\) ids per query where \(K\) is random in `MIN..=MAX`, splitting them into TRIGGER/CONDITION/OUTPUT, then computing:\n+  - `(TRIGGER union)` ∩ `(CONDITION union)` ∩ `(OUTPUT union)`\n+  - empty categories are ignored\n+  - example: `--multi-ids 6-12`
- `--seed <U64>`: RNG seed (if omitted, a random-ish seed is chosen and printed in the report)
- `--warmup <N>`: number of warmup queries (execute operations but don’t record stats)
- `--json-out <PATH?>`: optional machine-readable output (durations + config)
- `--print-samples <N?>`: optional: print first N query samples’ decoded references for sanity (kept off by default to avoid IO noise)

## Implementation approach

### 1) Add a new module

- Create `[src/bench_query.rs](./src/bench_query.rs)`.
- Public entry function:
  - `pub fn run(index_dir: &Path, set: &str, opts: BenchOptions) -> Result<()>`

### 2) Load shared data once

Inside `bench_query::run`:

- `set_dir = index_dir.join(set)`
- Load `Catalog` once: `Catalog::load(set_dir.join("catalog.json"))`
- Read `cards.bin` once into memory: `cards_data = std::fs::read(set_dir.join("cards.bin"))?` (so list timings include decode work, not file I/O).
- Load `idgd_catalog.json` once and preload the full pool of bitmaps (no `--pool-size`):\n+  - For each `entries[].id_gd` in `idgd_catalog.json`, try to load `set_dir/id_gd/<id>.roar`.\n+  - Skip missing or empty bitmaps.\n+  - Store each entry as `(id_gd, element_type, bitmap)`.

### 3) Random query selection (no new dependency)

- Implement a tiny deterministic RNG (e.g., xorshift64*) in `bench_query.rs` to avoid adding `rand` just for sampling.
- Sampling strategy:
  - **Single-id mode** (default): pick one random pool entry and run the three timed ops against that bitmap.\n+  - **Multi-id mode** (`--multi-ids MIN-MAX`):\n+    - pick \(K\) distinct ids (uniformly from the pool), where \(K\) is random in `MIN..=MAX`\n+    - split picked ids into TRIGGER/CONDITION/OUTPUT buckets using `element_type`\n+    - compute `(union within bucket)` then `intersect across non-empty buckets`\n+    - run the three timed ops against the resulting bitmap (includes union/intersect time)

### 4) Timing & stats

- Use `std::time::Instant` per operation.
- Collect per-op durations as nanoseconds internally (from `Instant`), but report summary stats in **milliseconds**.
- At end, compute summary stats per op:
  - count, mean, min, max, p50, p95 (simple sort + index).
- Print a concise report to stdout.

### 5) Decoding “uniques”

- For list operations, build a full object per card index:\n+  - `reference`: `catalog.decode_bit(bit)?.reference`\n+  - `stats`: from `CompactCardView` (`main_cost`, `recall_cost`, `mountain/ocean/forest_power`)\n+  - `effects_raw`: 3× main groups + echo group as `[trigger, condition, output]` idGd triplets (from `main_effect_group(g)` and `echo_effect()`)\n+  - `effects_text` (optional but included by default since you asked for “all abilities and card effects”): translate each non-zero idGd in each group using `text_by_id`, then join into human-readable lines (same grouping rules used in `query_id_gd_effect_text`)\n+\n+This keeps the list timings realistic by including:\n+- `cards.bin` record decode\n+- per-card reference decode\n+- per-card effect object construction\n+\n+Ensure the reference format matches `Catalog::decode_bit`:

```166:193:./src/catalog.rs
    pub fn decode_bit(&self, bit: u32) -> Result<DecodedCard> {
        let family = self
            .families
            .iter()
            .rfind(|f| f.start_bit <= bit)
            .ok_or_else(|| anyhow::anyhow!("bit {bit} is below first family"))?;

        let unique_id = bit - family.start_bit + 1;
        if unique_id > family.max_unique_id {
            anyhow::bail!(
                "bit {bit} falls in padding after family {} (max UniqueID {})",
                family.family_id,
                family.max_unique_id
            );
        }

        let set = family.source_set.as_deref().unwrap_or(&self.set);
        Ok(DecodedCard {
            reference: format!(
                "ALT_{}_B_{}_{}_U_{}",
                set, family.faction, family.family_number, unique_id
            ),
            unique_id,
            family_id: family.family_id.clone(),
            faction: family.faction.clone(),
            family_number: family.family_number.clone(),
        })
    }
```

### 6) Wire into CLI

- In `[src/lib.rs](./src/lib.rs)`, add `pub mod bench_query;`.
- In `[src/cli.rs](./src/cli.rs)`:
  - Add a `Command::BenchQuery { ... }` variant.
  - In the `match`, call `bench_query::run(...)`.

### 7) (Optional) JSON output

If `--json-out` is set:

- Write a small JSON struct containing:
  - config (set, queries, seed, warmup, mode, pool_len, multi_ids)
  - per-op summary stats
  - optionally raw durations (guard with a flag if size is a concern)

## How you’ll run it

Example:
- Single-id: `alt-indexer bench-query --index-dir <INDEX_ROOT> --set COREKS --queries 5000 --seed 1 --warmup 5`
- Multi-id: `alt-indexer bench-query --index-dir <INDEX_ROOT> --set COREKS --queries 5000 --multi-ids 6-12 --seed 1 --warmup 5`
- Random seed: `alt-indexer bench-query --index-dir <INDEX_ROOT> --set COREKS --queries 5000 --warmup 5`

(Works the same for merged indexes: pass the merged folder name as `--set`.)