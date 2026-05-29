# Plan 02: Effects catalog + autocomplete filters

## Goal

Load `GET /api/v2/effects` at startup and replace plain idGd text inputs with autocomplete comboboxes. Each field only shows its type: triggers on `t`, conditions on `c`, output on `o`. Values remain comma-separated idGd strings for the existing query builder.

## API

- Endpoint: `GET /api/v2/effects` ([`uniques-http-api/docs/api-spec.md`](../../uniques-http-api/docs/api-spec.md))
- Response: `{ triggers, conditions, output }` with `idGd`, locale `text` maps, `isEcho` / `isMain` on trigger/condition rows
- Dev proxy: `/api` → `http://127.0.0.1:8234` in [`vite.config.ts`](../vite.config.ts)

## Implementation

| Piece | File |
| ----- | ---- |
| Types | `src/types.ts` — `EffectCatalogItem`, `EffectsCatalogResponse` |
| Fetch hook | `src/hooks/useEffectsCatalog.ts` — one fetch on mount |
| Combobox | `src/components/EffectIdCombobox.tsx` — filter, comma-token select, a11y |
| Slot fields | `src/components/EffectSlotFields.tsx` — three comboboxes per slot |
| Panel | `src/components/FilterPanel.tsx` — pass catalog, locale, status |
| App | `src/App.tsx` — wire hook + locale |

## Combobox behavior

- Options scoped by parent (never mix types)
- Active token = text after last comma; filter by id prefix or localized label
- On select: replace token with `idGd`, keep prior comma-separated ids
- Manual typing still works if catalog fails to load

## Effect slot indexing in query params

Empty UI boxes (no t, c, or o) are omitted when building the URL. Active slots are compacted to sequential API indices: if box 0 is empty and box 1 has `c=166`, the query uses `effect[0][c]=166`, not `effect[1][c]`. Implemented in `buildQuery.ts` via `slotHasValues` + a running `effectIndex`.

## Deferred

- Filter support fields by `isEcho` only
- Automated UI tests

## Manual test

1. Run API with index + `npm run dev` in `demo-ui`
2. One `GET /api/v2/effects` on load
3. Trigger/condition/output boxes show type-appropriate suggestions
4. Selection updates `effect[N][t|c|o]` query params
5. Locale switch updates suggestion labels

## Status

Implemented:

- `src/types.ts` — catalog types
- `src/hooks/useEffectsCatalog.ts`
- `src/components/EffectIdCombobox.tsx`
- `src/components/EffectSlotFields.tsx`, `FilterPanel.tsx`, `App.tsx`
