## Plan 12: `withFamilies` on card search

### Goal

When `withFamilies` is present on **`GET /api/v2/cards`** (and **`cursor` is omitted**), extend the response with a `families` array:

- One entry per **logical** `family_id` that has at least one matching card
- **`count`**: number of matching `card_index` values in that family’s merged span(s)
- **`reference`**: full reference id of the **first** matching card in that family (lowest `card_index` in the merged span)
- **`name`**: localized character name (`BTreeMap<String, String>`, same shape as `CardV2.name`)
- No paging/cursors for `families` (computed once per request)
- **`cards[]`** on that response: one full `CardV2` per `families[].reference` only (`limit` ignored; no `iter.cursor`). Follow-up requests without `withFamilies` use normal card paging.

**Status:** implemented.

Production index: **527** catalog rows, **418** unique `family_id` values after CORE+COREKS grouping (`ALL_SETS/catalog.json`).

### Family span groups (load time)

Consecutive catalog rows with the same `family_id` and touching `start_bit`/`range_end` are merged (CORE+COREKS overlap). Stored as `FamilySpanGroup { family_id, range_start, range_end }` on `AppState`.

### Query parameter

| Param | Behavior |
| --- | --- |
| `withFamilies` | Optional flag (presence). When set and **`cursor` is omitted**, response includes `families`. Ignored when `cursor` is present. |

### Response

```json
{
  "iter": { "total": 1200 },
  "families": [
    {
      "familyId": "AX_05",
      "count": 42,
      "reference": "ALT_COREKS_B_AX_05_U_1",
      "name": { "en_US": "...", "fr_FR": "..." }
    }
  ],
  "cards": [
    { "reference": "ALT_COREKS_B_AX_05_U_1", "name": { ... }, "set": { ... }, ... }
  ]
}
```

`iter.total` is the global match count (`bitmap.len()`), not the number of families. `cards.length` equals `families.length`.

### Computation

Per `FamilySpanGroup`:

1. `count` = `bitmap.range_cardinality(range_start..range_end)`
2. Skip if `count == 0`
3. First match = `bitmap.range(range_start..range_end).next()` (not `iter().find` on the full bitmap)
4. `reference` = `decode_reference(card_index)`; `name` = `family_for_bit(card_index).name`

Lowest `card_index` in a merged CORE+COREKS span is typically the COREKS print.

### Files

- [`src/loader.rs`](../src/loader.rs) — `FamilySpanGroup`, `build_family_span_groups`
- [`src/state.rs`](../src/state.rs) — store + accessor
- [`src/cards.rs`](../src/cards.rs) — parse, `FamilyMatchV2`, `families_from_bitmap`, handler
- [`docs/api-spec.md`](../../docs/api-spec.md) — param + response

### Out of scope

- Paging `families`; `count: 0` rows; full `CardV2` in `families[]`; `debug_bga_trigram` on family rows
