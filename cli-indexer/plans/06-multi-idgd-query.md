## Plan: Multi-idGd Query

## Goal
Update the `Query` subcommand so users can provide multiple `idGd` values in one invocation. The tool should:

- Split provided `idGd`s into **Trigger**, **Condition**, and **Output** buckets.
- Compute **UNION** of bitmaps within each bucket.
- Compute **INTERSECT** of the resulting bucket bitmaps (skipping any empty bucket).
- When `--show_effect` is used, print a recap of the selected `idGd` texts grouped by bucket before listing cards.

## Current state (what we’ll change)
- CLI currently accepts a single `id_gd: u32`.
- The index already tags each `idGd` with `element_type` (e.g. `TRIGGER`, `CONDITION`, `OUTPUT`) in `idgd_catalog.json`.

## Approach
### 1) CLI: accept multiple `idGd`
- Change `id_gd: u32` to `id_gd: Vec<u32>` in `src/cli.rs`.
- Implement comma-separated parsing: `--id-gd 24,191,76`.
- Keep the flag name `--id-gd` the same.

### 2) Build a grouped query spec from `idgd_catalog.json`
In `src/query.rs`:
- Load and parse `idgd_catalog.json` (similar to existing `query_id_gd_effect_text`).
- Build a lookup map `id_gd -> (element_type, translations)`.
- For each provided `idGd`, look up its `element_type`:
  - `TRIGGER` → add to `triggers`
  - `CONDITION` → add to `conditions`
  - `OUTPUT` → add to `outputs`
  - Otherwise: error.

### 3) Bitmap math: union-within, intersect-across
- For each non-empty bucket, load each `id_gd/<id>.roar` bitmap and OR them together.
- If 2–3 buckets are non-empty, AND the bucket bitmaps together.
- If only 1 bucket is non-empty, that bucket bitmap is the final result.
- If the user passes 0 ids (or all ids fail classification), error.

Semantics:
- **Triggers union** ∩ **Conditions union** ∩ **Outputs union**
- Buckets with no ids are ignored (not intersected).

### 4) Output changes
#### Non-`--show_effect` path
- Keep the existing table output, but update the header line to describe the multi-id query.
- Listing (`--list`) should iterate the *final* bitmap and show card rows (same as today).

#### `--show_effect` path: recap selected ids
Before printing any matching cards, print:
- `Searching for cards matching:`
- Then, for each non-empty bucket, print:
  - `Triggers is one of :` then each selected trigger text (localized via `--locale`, with fallback)
  - Similarly for Condition / Output

### 5) API shape / refactor points
- Add new query entrypoints in `src/query.rs`, leaving the single-id functions as wrappers around the new multi-id implementation.

### 6) Error handling
- If any supplied `idGd` is missing from `idgd_catalog.json` or has an unknown `element_type`, return a clear error.

## Files to change
- `src/cli.rs`: change `Query` args (`id_gd: Vec<u32>` with comma delimiter) and call new query functions.
- `src/query.rs`: implement multi-id grouping, bitmap union/intersection, and `--show_effect` recap output support.

## Test plan
- Run `alt-indexer query --index-dir ... --set ... --id-gd <single>` and confirm output matches prior behavior.
- Run `alt-indexer query --id-gd <trigger1>,<trigger2>` and confirm it returns union.
- Run `alt-indexer query --id-gd <trigger>,<condition>` and confirm it returns intersection of unions.
- Run `--show_effect` and verify the recap block prints the selected texts in each category before card output.
- Run with an unknown `idGd` and confirm it errors with a clear message.

