---
name: 07-per-ability-bitmaps
overview: Per-effect-line Roaring bitmaps, nested `idgd_catalog.json` metadata (including `is_echo`), and query/bench-query that default to same-line matching via per-line sub-indexes (`--whole-card` for legacy combined `{id}.roar`).
todos:
  - id: explore-current-build
    content: Locate and understand current whole-card id_gd bitmap build + idgd_catalog writer paths.
    status: completed
  - id: add-per-line-store
    content: Design and add a bitmap store keyed by (id_gd, line) with a write_dir method producing *_m1/*_m2/*_m3/*_ec files.
    status: completed
  - id: populate-per-line-bitmaps
    content: Update build indexing loop to insert id_gd values into the correct per-line bitmaps using CompactCardFields main_effect/echo_effect slots.
    status: completed
  - id: extend-idgd-catalog
    content: Extend idgd_catalog.json schema to include optional nested m1/m2/m3/ec bitmap metadata entries and write them only when bitmap exists.
    status: completed
  - id: update-merge-and-readers
    content: Update merge.rs and readers for extended catalog schema; query/bench-query default to per-line bitmaps with --whole-card for combined index.
    status: completed
  - id: add-tests
    content: Add minimal tests ensuring per-line bitmaps and catalog nesting are emitted correctly and empty ones are omitted.
    status: completed
  - id: is-echo-catalog-field
    content: Add `is_echo` (true/false/null) to idgd_catalog entries; log build error when idGd appears in both MAIN and ECHO.
    status: completed
isProject: false
---

## Goal
Add **per-effect-line** (`MAIN_EFFECT` group 1..3 and `ECHO_EFFECT`) sub-bitmaps in `id_gd/` in addition to the existing whole-card `id_gd/<id>.roar` bitmap.

- **Existing**: `id_gd/<id_gd>.roar` → cards where `id_gd` appears anywhere on the card.
- **New**: `id_gd/<id_gd>_m1.roar`, `_m2.roar`, `_m3.roar`, `_ec.roar` → cards where `id_gd` appears in that specific effect line (any of trigger/condition/output within that line).
- **Sparsity**: If a per-line bitmap is empty, **do not write it** and **do not include it** in `idgd_catalog.json`.

## Current implementation (what we’ll build on)
- `build` indexes cards in [`alt-indexer/src/build.rs`](alt-indexer/src/build.rs) and currently inserts whole-card occurrences using:
  - [`effects_from_card()`](alt-indexer/src/card.rs) which dedupes `id_gd` across the entire card.
  - `BitmapStore` keyed by `id_gd` in [`alt-indexer/src/bitmap.rs`](alt-indexer/src/bitmap.rs).
- Effect-line structure already exists in the compact extraction:
  - `CompactCardFields.main_effect[[u16;3];3]` and `echo_effect[u16;3]` in [`alt-indexer/src/compact.rs`](alt-indexer/src/compact.rs).

## Catalog shape change (nested sub-entries)
Update [`alt-indexer/src/idgd_catalog.rs`](alt-indexer/src/idgd_catalog.rs) schema so each top-level entry keeps the existing fields, and gains optional nested objects for each per-line bitmap.

Proposed JSON shape per entry (example):

```json
{
  "id_gd": 1,
  "card_count": 42,
  "bitmap_bytes": 1234,
  "bitmap_file": "1.roar",
  "element_type": "TRIGGER",
  "is_echo": false,
  "translations": { "en_US": {"locale":"en_US","text":"..."} },
  "m1": { "card_count": 30, "bitmap_bytes": 900, "bitmap_file": "1_m1.roar" },
  "m2": { "card_count": 12, "bitmap_bytes": 400, "bitmap_file": "1_m2.roar" },
  "m3": null/omitted,
  "ec": { "card_count": 5, "bitmap_bytes": 120, "bitmap_file": "1_ec.roar" }
}
```

Top-level fields (in addition to existing):

- **`is_echo`** (`boolean` or `null`):
  - `false` — idGd appears only under **MAIN_EFFECT** effect lines (`m1`..`m3`).
  - `true` — idGd appears only under **ECHO_EFFECT** (`ec`).
  - `null` — idGd appears in **both** regions; build logs  
    `error: idGd {id} appears in both MAIN_EFFECT and ECHO_EFFECT`.

Nested per-line objects (`m1`, `m2`, `m3`, `ec`) use the same metadata triple:
- `card_count`
- `bitmap_bytes`
- `bitmap_file`

Omit nested keys when the corresponding per-line bitmap was not written (empty).

## Implementation plan
### 1) Add an “id_gd + effectLine” bitmap store
- Add a small new store type (either in [`alt-indexer/src/bitmap.rs`](alt-indexer/src/bitmap.rs) or a sibling module) keyed by `(id_gd, line)` where `line ∈ {m1,m2,m3,ec}`.
- Provide:
  - `insert(id_gd, line, card_index)`
  - `iter()`
  - `write_dir(&id_gd_dir)` which writes files named:
    - `{id_gd}_m1.roar`, `{id_gd}_m2.roar`, `{id_gd}_m3.roar`, `{id_gd}_ec.roar`
  - Return a size map keyed by `(id_gd, line)` so catalog can record `bitmap_bytes`.
- Ensure it **skips empties** by construction (don’t create an entry until first insert), and/or by checking `is_empty()` before writing.

### 2) Populate the per-line store during build
- In [`alt-indexer/src/build.rs`](alt-indexer/src/build.rs), extend the build loop to keep both:
  - existing `BitmapStore` (whole-card)
  - new per-line store
- In `apply_card_index()`, walk [`id_gds_per_effect_line()`](alt-indexer/src/card.rs) (all `cardEffectElements` on each `MAIN_EFFECT` / `ECHO_EFFECT` display):
  - insert each `(line, id_gd)` into `PerLineBitmapStore`
  - call `IdGdCatalogBuilder.record_effect_line(id_gd, line)` to track main vs echo
- Continue to record `IdGdCatalogBuilder.record_first(occ)` from whole-card `effects_from_card()` (preserves `element_type` + `translations`).

### 3) Write new bitmaps and extend `idgd_catalog.json`
- In `write_index_outputs()` in [`alt-indexer/src/build.rs`](alt-indexer/src/build.rs):
  - Write existing whole-card bitmaps first (current behavior).
  - Write new per-line bitmaps to the same `id_gd/` directory.
- Extend [`alt-indexer/src/idgd_catalog.rs`](alt-indexer/src/idgd_catalog.rs):
  - Add nested optional fields `m1`, `m2`, `m3`, `ec` to `IdGdCatalogEntry`.
  - Add a helper struct like `BitmapMeta { card_count, bitmap_bytes, bitmap_file }`.
  - Update `IdGdCatalogBuilder::build(...)` signature to accept:
    - the existing whole-card `BitmapStore` + bytes map
    - the per-line store (or at least a way to look up `(id_gd, line)` bitmap cardinality and bytes)
  - For each `id_gd` present in the whole-card store:
    - set the top-level fields exactly as today
    - set `is_echo` from `seen_main` / `seen_echo` flags (`is_echo_from_flags`)
    - attach each nested `m1/m2/m3/ec` only if that bitmap exists/non-empty (and therefore was written).

### 3b) `is_echo` on catalog entries (implemented)
- During build, `record_effect_line` sets `seen_main` for `m1`..`m3` and `seen_echo` for `ec`.
- At catalog finalize, `is_echo_from_flags` maps to `false` / `true` / `null` (with stderr error on conflict).
- Merge combines `is_echo` across source catalogs via `merge_is_echo_values` (conflicts → `null` + error log).

### 4) Query / bench-query: per-line sub-indexes by default (`--whole-card` opt-in)

**CLI flags** (both `query` and `bench-query`):

| Flag | Default | Behavior |
|------|---------|----------|
| *(none)* | — | Use per-line sub-indexes `{id_gd}_m1/_m2/_m3/_ec.roar` |
| `--whole-card` | `false` | Use combined whole-card index `{id_gd}.roar` (legacy) |

**Default (per-line) query semantics** — constraints must match on the **same effect line**:

For each line in `m1`, `m2`, `m3`, `ec`:

1. Within the line, for each non-empty bucket in the query (TRIGGER / CONDITION / OUTPUT):
   - Union the per-line bitmaps for ids in that bucket (e.g. `24_m1.roar`, `191_m1.roar`).
   - Missing files are treated as empty (no error).
2. Intersect buckets **on that line** (e.g. trigger ∪ then ∩ condition ∪).
3. Union line results across `m1`…`ec`.

Example: `--id-gd 24,191` (trigger 24, condition 191) matches only cards where **both** ids appear on **one** line (e.g. `m1` has trigger 24 and condition 191). It does **not** match if trigger 24 is on `m1` and condition 191 is on `m2`.

**`--whole-card` (legacy)** — uses `{id_gd}.roar` only:

- Union all trigger ids globally → one group.
- Union all condition ids globally → one group.
- Union all output ids globally → one group.
- Intersect the groups (same as pre–per-line behavior).

**Implementation** ([`alt-indexer/src/query.rs`](alt-indexer/src/query.rs)):

- `execute_idgd_query` / `execute_idgd_query_preloaded` shared by query and bench.
- `IdGdQueryBuckets` holds deduped trigger/condition/output id lists (element type from `idgd_catalog.json` top-level entries).
- Catalog nested `m1`/`m2`/`m3`/`ec` fields are metadata only; query loads bitmaps from disk by filename suffix.

**Bench-query** ([`alt-indexer/src/bench_query.rs`](alt-indexer/src/bench_query.rs)):

- `BenchOptions.whole_card` mirrors CLI.
- Preloads either all `{id}_m*.roar` / `{id}_ec.roar` (default) or `{id}.roar` (`--whole-card`).
- Multi-id mode samples random catalog ids and runs the same bucket/line algorithm as `query` (not the old pool ∩ across unrelated per-line files).

**Schema compatibility**: `IdGdCatalogEntry` includes `is_echo` and optional nested `m1`/`m2`/`m3`/`ec`. Top-level `id_gd`, `element_type`, and `translations` remain the source of truth for query bucketing. Rebuild indexes after upgrading `alt-indexer` so per-line `.roar` files and `is_echo` are present.

### 5) Update `merge` step (follow-up inside same change so builds don’t regress)
Even though you said “update build step first”, the repo will stay consistent if `merge` also propagates these files and writes them into the merged `ALL_SETS/id_gd/` folder.
- In [`alt-indexer/src/merge.rs`](alt-indexer/src/merge.rs):
  - Extend `merge_id_gd()` to also merge per-line bitmaps for each `(id_gd, line)` by applying the same `Mapping` remap logic used for `{id}.roar`.
  - Write merged `{id_gd}_{line}.roar` files only when non-empty.
  - Extend `write_idgd_catalog()` to populate nested `m1/m2/m3/ec` and merged `is_echo`.

### 6) Tests / validation additions
- Add a focused unit/integration test (likely in `alt-indexer/tests/build_tmp.rs`) that builds a tiny fixture index with known `MAIN_EFFECT` groups and verifies:
  - `{id}.roar` contains cards regardless of which line the id appears in.
  - `{id}_m1.roar` vs `{id}_m2.roar` split correctly.
  - empty line bitmaps are not written and nested catalog fields are absent.

## Files expected to change
- [`alt-indexer/src/build.rs`](alt-indexer/src/build.rs)
- [`alt-indexer/src/bitmap.rs`](alt-indexer/src/bitmap.rs) (or new module for the per-line store)
- [`alt-indexer/src/idgd_catalog.rs`](alt-indexer/src/idgd_catalog.rs)
- [`alt-indexer/src/query.rs`](alt-indexer/src/query.rs)
- [`alt-indexer/src/bench_query.rs`](alt-indexer/src/bench_query.rs)
- [`alt-indexer/src/cli.rs`](alt-indexer/src/cli.rs) (`--whole-card` on `query` and `bench-query`)
- [`alt-indexer/src/merge.rs`](alt-indexer/src/merge.rs)
- [`alt-indexer/tests/build_tmp.rs`](alt-indexer/tests/build_tmp.rs)

## Naming conventions
- **Directory**: still under `id_gd/`
- **Files**:
  - Whole: `{id_gd}.roar`
  - Per-line: `{id_gd}_m1.roar`, `{id_gd}_m2.roar`, `{id_gd}_m3.roar`, `{id_gd}_ec.roar`
- **Catalog nested field names**: `m1`, `m2`, `m3`, `ec` (matching the file suffixes).
- **Catalog `is_echo`**: `false` = main only, `true` = echo only, `null` = both (error at build).