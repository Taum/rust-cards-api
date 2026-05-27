# Stats indexer (4-bit card stats)

## Goal

During `build`, index the five numeric stats already stored in `cards.bin` (from `cardElements`) as **Roaring bitmaps** keyed by stat value `0..=15`. Write a **`stats_summary.json`** next to `catalog.json` with per-value card counts for CLI tables and quick inspection.

Stats indexed:

| `cards.bin` field   | JSON `cardElementType.reference` |
|---------------------|----------------------------------|
| `main_cost`         | `MAIN_COST` (hand cost)          |
| `recall_cost`       | `RECALL_COST`                    |
| `mountain_power`    | `MOUNTAIN_POWER`                 |
| `ocean_power`       | `OCEAN_POWER`                    |
| `forest_power`      | `FOREST_POWER`                   |

Bit positions use the same **`card_index`** as `catalog.json` and `id_gd/*.roar`.

## Output layout

```text
<out>/<SET>/
  catalog.json
  stats_summary.json          # counts per field × value
  stats/
    main_cost/
      00.roar … 15.roar       # only non-empty buckets written
    recall_cost/
      …
    mountain_power/
    ocean_power/
    forest_power/
  cards.bin
  id_gd/
    …
```

Bitmap file naming: two-digit value, e.g. `stats/main_cost/07.roar` = cards with `main_cost == 7`.

## Build integration

Single pass with the existing crawl (no extra JSON read):

1. After `extract_compact_fields`, call `StatIndexBuilder::insert(card_index, &compact)`.
2. At end of build (with `id_gd` write), call `stat_index.write_dir(&set_out.join("stats"))`.
3. Write `stats_summary.json` from bucket cardinalities.

## `stats_summary.json` shape

```json
{
  "version": 1,
  "set": "COREKS",
  "total_cards_indexed": 12345,
  "fields": [
    {
      "field": "main_cost",
      "element_reference": "MAIN_COST",
      "counts": [0, 0, 42, 100, …],
      "bitmap_dir": "stats/main_cost"
    }
  ]
}
```

- `counts[i]` = number of cards with stat value `i` (length 16).
- `bitmap_dir` is relative to the set output directory.

## Queries (future CLI)

| Operation | Implementation |
|-----------|----------------|
| EQ(v)     | Load `stats/<field>/{v:02}.roar` |
| LT(v)     | OR buckets `0 .. v-1` |
| GT(v)     | OR buckets `v+1 .. 15` |
| AND idGd  | `bitmap_id_gd & bitmap_stat` |

`cards.bin` remains the row store for listing columns; stat bitmaps are the column index.

## Related

- [idgd-compact-card-format.md](idgd-compact-card-format.md) — `cards.bin` header bytes 1–5
- [idgd-bitset-indexer.md](idgd-bitset-indexer.md) — `card_index` and catalog
