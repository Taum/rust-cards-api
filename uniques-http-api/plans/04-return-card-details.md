## Plan 04: `GET /api/v2/cards` — return full card details (v2 response shape)

### Goal

Expand the `GET /api/v2/cards` endpoint so it returns the **full per-card JSON model** drafted in `docs/api-spec.md`, rather than just `{ "reference": ... }`.

This keeps the filtering + paging behavior from Plan 03, but enriches the returned card objects with:

- `reference`
- stats (`mainCost`, `recallCost`, `forestPower`, `mountainPower`, `oceanPower`)
- `faction: { code }`
- `mainEffect` and `echoEffect` as locale keyed text maps

### Data sources used at runtime

This implementation intentionally uses **only data already loaded into memory** in `AppState`:

- `cards.bin` → decoded via `AppState.card_view(card_index)` which returns `alt_indexer::compact::CompactCardView`
  - provides: faction code, costs/powers, and compact effect idGd triplets
- `catalog.json` → used by `AppState.decode_reference(card_index)` to produce the `reference` string
- `idgd_catalog.json` → used for:
  - request validation (Plan 03)
  - effect text localization in the response (Plan 04)

No per-request JSON card file reads are introduced by this change.

### Response shape

Each returned card in `cards[]` now includes (camelCase JSON keys):

```json
{
  "reference": "ALT_COREKS_AX_05_U_161",
  "mainCost": 2,
  "recallCost": 3,
  "forestPower": 1,
  "mountainPower": 6,
  "oceanPower": 3,
  "faction": { "code": "AX" },
  "mainEffect": {
    "en_US": "...",
    "fr_FR": "..."
  },
  "echoEffect": {
    "en_US": "...",
    "fr_FR": "..."
  }
}
```

Notes:
- `mainEffect` is derived from the three compact main effect groups (M1/M2/M3) on the card record.
- `echoEffect` is derived from the compact echo/support group (Ec) on the card record.

### Implementation outline

#### 1) Response structs and JSON casing

In `uniques-http-api/src/cards.rs`:

- Replace the old `CardRef { reference }` response element with a richer `CardV2` struct.
- Add `#[serde(rename_all = "camelCase")]` on `CardV2` so Rust snake_case fields serialize as:
  - `main_cost` → `mainCost`
  - `main_effect` → `mainEffect`
  - `echo_effect` → `echoEffect`

#### 2) Building `CardV2` from the compact record

During paging, for each `card_index` selected from the match bitmap:

- Decode `reference` using `state.decode_reference(card_index)`.
- Decode the compact record using `state.card_view(card_index)` and populate:
  - `mainCost`, `recallCost`, `forestPower`, `mountainPower`, `oceanPower`
  - `faction.code` (mapped from the compact `faction_code`)

This preserves Plan 03’s cursor semantics: return entries with `card_index > cursor`, and return `iter.cursor` only when another page may exist.

#### 3) Localized effect text maps

The compact record stores only idGd ids for the effect slot projection:

- `main_effect_group(0..2)` → three main groups, each `[trigger, condition, output]`
- `echo_effect()` → one echo/support group `[trigger, condition, output]`

To build `mainEffect` and `echoEffect`:

- Build an in-memory lookup `idgd_by_id: BTreeMap<u32, &IdGdCatalogEntry>` from `state.idgd_catalog().entries`.
- For each locale:
  - choose the localized string for each id via `entry.translations`:
    - prefer requested locale
    - fallback to `en_US`
    - fallback to “first available translation”
  - produce one line per group as `"TRIGGER CONDITION OUTPUT"` (skipping empty ids / empty strings)
- `mainEffect` joins multiple non-empty group lines with a **double space** (`"  "`), not commas.

Rationale: keep the response stable and human-readable while avoiding extra nested objects until we decide on a more structured “effects” model.

### Files changed

- `uniques-http-api/src/cards.rs`
  - new `CardV2` response model
  - paging now builds full card objects
  - localized effect map construction
  - main effect group joiner changed to `"  "`

### Verification

- `cargo test` in `uniques-http-api/` passes after the change.

