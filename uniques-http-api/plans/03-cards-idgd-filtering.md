## Plan 03: `GET /api/v2/cards` — idGd ability filtering (first slice)

### Goal

Add a first, small slice of the drafted API: **filter cards by idGd occurrences in abilities** and return a minimal list of card references with cursor paging.

### Inputs / index data

The server already loads a merged index into `AppState` (see `INDEX_PATH` bootstrap), including:

- `manifest.json` → `AppState.manifest()` (for `total_bit_span` / validation)
- `idgd_catalog.json` → `AppState.idgd_catalog()` (for idGd type validation)
- per-line idGd roaring bitmaps → `AppState.id_gd_per_line(): BTreeMap<(u32, EffectLine), RoaringBitmap>`

### Endpoint

`GET /api/v2/cards`

### Query parameters (first iteration)

Ability filters:
- `effect[0][t]=<idGd or comma-list>`
- `effect[0][c]=<idGd or comma-list>`
- `effect[0][o]=<idGd or comma-list>`
- `support[t]=<idGd or comma-list>`
- `support[c]=<idGd or comma-list>`
- `support[o]=<idGd or comma-list>`

Notes:
- Only `effect[0]` is supported initially; reject `effect[1]`, `effect[2]`, … as unsupported.
- Values can be a single integer (`90`) or comma-separated list (`90,41,234`).

Paging:
- `limit` (default **50**): must be **1..=200**, otherwise throw a 400 error.
- `cursor` (optional u32): a raw `card_index` in full index space.
  - Validate `cursor < manifest.total_bit_span`, otherwise throw a 400 error.
  - Results include matches with `card_index > cursor`.

### Type validation (strict)

The endpoint must reject an idGd if the selector key doesn’t match its `element_type` from `idgd_catalog.json`:
- `[t]` only accepts `TRIGGER`
- `[c]` only accepts `CONDITION`
- `[o]` only accepts `OUTPUT`

Unknown idGd (missing from `idgd_catalog`) is also rejected (400).

### Matching semantics (important)

This is intentionally aligned with the `alt-indexer query` default behavior (per-line query, not whole-card).

#### `effect[0]` (main effect only)

Search space: **main effect lines only**: `EffectLine::M1`, `EffectLine::M2`, `EffectLine::M3`.
Do **not** search `EffectLine::Ec` for `effect[0]`.

Per-line predicate:

- For a given line `L`, compute:
  - `T_L = union of bitmaps for all ids in effect[0][t] on line L`
  - `C_L = union of bitmaps for all ids in effect[0][c] on line L`
  - `O_L = union of bitmaps for all ids in effect[0][o] on line L`
  - `lineMatch(L) = (T_L if non-empty) ∩ (C_L if non-empty) ∩ (O_L if non-empty)`

Then:
- `effect0Match = lineMatch(M1) OR lineMatch(M2) OR lineMatch(M3)`

Consequence:
- `effect[0][t]=1&effect[0][o]=90` requires **a single main-effect line** that contains **both** idGd 1 (trigger) and idGd 90 (output). It is **not** “trigger anywhere AND output anywhere” across different lines.

#### `support[...]` (echo/support only)

Search space: `EffectLine::Ec` only.

Per-line predicate:
- Same bucket logic as above, but only for `Ec`:
  - `supportMatch = (union triggers on Ec) ∩ (union conditions on Ec) ∩ (union outputs on Ec)` (skipping empty buckets)

#### Combining `effect[0]` and `support[...]`

If both are provided:
- `finalMatch = effect0Match ∩ supportMatch`

### Response (minimal)

```json
{
  "iter": { "total": 51513, "cursor": 10516 },
  "cards": [
    { "reference": "ALT_COREKS_B_AX_04_U_187" }
  ]
}
```

- `iter.total`: cardinality of the final match bitmap.
- `iter.cursor`: last returned `card_index` if `limit` was reached; omitted/null if there is no next page.

### Implementation outline (files)

- Add handler module: `uniques-http-api/src/cards.rs`\n
  - parse/validate query params\n
  - validate idGd types using `AppState.idgd_catalog()`\n
  - compute roaring bitmap with semantics above\n
  - apply cursor/limit paging and decode references via `AppState.decode_reference()`
- Wire route: `uniques-http-api/src/lib.rs`\n
  - `Router::route("/api/v2/cards", get(get_cards_v2))`

### Verification

- Unit tests for:\n
  - type mismatch rejection\n
  - per-line bucket intersection behavior\n
  - OR across `M1|M2|M3`\n
  - cursor paging (`card_index > cursor`)
- Manual check against `alt-indexer query` default (no `--whole-card`):\n
  - Example: `effect[0][t]=1&effect[0][o]=90` should align with `alt-indexer query --id-gd 1,90` results.

