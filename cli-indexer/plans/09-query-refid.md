# Query by reference ID (`--refid`)

## Goal

```bash
alt-indexer query --index-dir ./full_index --set ALL_SETS \
  --refid ALT_COREKS_B_AX_04_U_10 --locale en_US
```

Output matches the per-card block from `--show-effect` (no idGd recap block):

```
query: 1 cards

ALT_COREKS_B_AX_04_U_10
Cost: 2 / 0          Power: O:1 / M:0 / F:0
<translated effect lines>
-----------------
```

`--refid` is mutually exclusive with `--id-gd`, `--list`, `--show-effect`, and `--whole-card`. Only `--locale` may be combined.

## Data flow

1. Parse `--refid` with `parse_card_reference` (`path.rs`).
2. Resolve global bit via `Catalog::lookup_bit` (`catalog.json`).
3. Load `cards.bin` record and `idgd_catalog.json` translations.
4. Print stats + effect lines (same as `--show-effect` card block).

No `id_gd/` bitmaps are read.

## Lookup rules

- Match family by `faction`, `family_number`, and `source_set` (or catalog `set` when absent).
- `bit = start_bit + unique_id - 1`.
- Error if reference not in catalog, in padding (`unique_id > max_unique_id`), or slot has no indexed card (zero faction + zero tail bytes).

## Files

- `src/path.rs` — `parse_card_reference`
- `src/catalog.rs` — `lookup_bit`
- `src/query.rs` — `query_refid_effect_text`
- `src/cli.rs` — flag, arg group, handler
- `tests/build_tmp.rs` — integration test
