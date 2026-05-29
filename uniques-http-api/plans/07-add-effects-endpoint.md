# Plan 07: `GET /api/v2/effects` — effects list for filtering UI

## Goal

Add `GET /api/v2/effects` so clients can populate effect filter picklists (trigger / condition / output idGd values with localized text). The response shape is defined in [`docs/api-spec.md`](../docs/api-spec.md).

The endpoint takes no query parameters and returns a **static JSON body** built once at startup from `idgd_catalog.json`.

## Response shape

```json
{
  "triggers": [
    {
      "idGd": 1,
      "text": { "en_US": "{R}", "fr_FR": "{R}", ... },
      "isEcho": false,
      "isMain": true
    }
  ],
  "conditions": [ ... ],
  "output": [
    {
      "idGd": 193,
      "text": { "en_US": "[AFTER_YOU].", ... }
    }
  ]
}
```

Notes:

- Response keys use camelCase (`idGd`, `isEcho`, `isMain`).
- `triggers` and `conditions` include `isEcho` and `isMain` from catalog `EffectRegionFlags`.
- `output` entries omit region flags.
- `text` maps locale keys (`en_US`, `fr_FR`, …) to plain strings (from `LocaleText.text` in the catalog).

## Data source

Only [`idgd_catalog.json`](../../docs/ALL_SETS-index-format.md) (already loaded as `IdGdCatalog` in `AppState`):

| Catalog field | API usage |
| ------------- | --------- |
| `element_type` | Routes entry to `triggers`, `conditions`, or `output` (`TRIGGER` / `CONDITION` / `OUTPUT`) |
| `id_gd` | `idGd` |
| `translations` | `text` locale map |
| `is_main`, `is_echo` (catalog) | Serialized as `isMain`, `isEcho` on trigger/condition items only |

Unknown `element_type` values are skipped.

## Implementation (completed)

### 1) Serde types and builder — `src/effects.rs`

- `EffectsListResponse` — top-level `{ triggers, conditions, output }`.
- `EffectPartWithRegion` — trigger/condition row.
- `EffectPart` — output row.
- `build_effects_list(&IdGdCatalog)` — groups entries by `element_type`, sorts each list by `id_gd`.
- `serialize_effects_list` — one-shot `serde_json::to_vec` for memoization.

### 2) Startup memoization — `src/loader.rs`

After loading `idgd_catalog.json`:

1. `let effects_list = build_effects_list(&idgd_catalog);`
2. `let effects_body = Arc::new(serialize_effects_list(&effects_list)?);`
3. Store on `AppStateInner.effects_body: Arc<Bytes>`.
4. Log counts + JSON byte size at startup.

No per-request serialization.

### 3) AppState — `src/state.rs`

- `effects_body: Arc<Bytes>` on `AppStateInner`.
- `AppState::effects_body()` accessor.

### 4) HTTP route — `src/lib.rs`

```text
GET /api/v2/effects  →  effects::get_effects_v2
```

Handler clones the memoized `Bytes` and returns `Content-Type: application/json` (no `Json<T>` wrapper, avoids re-serialization).

### 5) Tests

| File | Coverage |
| ---- | -------- |
| `src/effects.rs` (unit) | Grouping by `element_type`, sort order, JSON field names |
| `tests/effects.rs` | Integration: status, content-type, payload shape, body identical to `state.effects_body()` |
| `tests/health.rs` | Health only (effects test split out) |
| `tests/load_index.rs` | Asserts `effects_body` non-empty after load |
| `tests/fixtures/minimal_index/idgd_catalog.json` | Sample trigger / condition / output entries |

`src/cards.rs` test helper `test_state()` updated to build `effects_body` when constructing `AppStateInner`.

## Files touched

| Path | Change |
| ---- | ------ |
| `src/effects.rs` | **New** — types, build, serialize, handler |
| `src/loader.rs` | Build + memoize effects JSON at index load |
| `src/state.rs` | `effects_body` field + accessor |
| `src/lib.rs` | Register route, `mod effects` |
| `src/cards.rs` | Test `AppStateInner` includes `effects_body` |
| `tests/effects.rs` | **New** — integration test |
| `tests/health.rs` | Effects test removed |
| `tests/load_index.rs` | Effects body assertion |
| `tests/fixtures/minimal_index/idgd_catalog.json` | Fixture entries for effects test |

## Deferred

- Demo UI: fetch `/api/v2/effects` for filter dropdowns (still manual idGd entry today).
- `ETag` / `Cache-Control` headers for CDN/browser caching of the static body.
- Optional query param to trim locales (would break “fully static” memoization unless we precompute variants).

## Verification

```bash
cd uniques-http-api
cargo test
```

Expected: unit tests in `effects::tests`, `tests/health`, `tests/effects`, `tests/load_index` all pass.
