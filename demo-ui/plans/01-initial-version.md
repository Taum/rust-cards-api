# Demo UI — initial version

Single-page React demo for `GET /api/v2/cards` ([`uniques-http-api`](../../uniques-http-api)).

## Goals

- Select filters and see live URL query params
- Auto re-query on change (300ms debounce) with response time
- Show results + raw JSON; placeholder hooks for a future card renderer library

## Stack

- Vite + React + TypeScript + Tailwind CSS
- Dev proxy: `/api` → `http://127.0.0.1:8234`

## Filters (v1)

| UI | API param | Notes |
|----|-----------|-------|
| Effect slots (dynamic) | `effect[N][t\|c\|o]` | Add/remove slots; comma-separated idGd. API today: slot 0 only (slot 1+ → 400 until backend updated). |
| Support | `support[t\|c\|o]` | Same encoding |
| Factions | `faction[]` | Checkboxes: AX, BR, LY, MU, OR, YZ |
| Hand cost | `mainCost` / `mainCost[]` | Text: `3`, `2,4,6`, `2-5`, `2-4,7` (0–15; `N-M` inclusive → expanded `[]`) |
| Reserve cost | `recallCost` / `recallCost[]` | Same syntax |
| Limit / cursor | `limit`, `cursor` | limit 1–200, default 50 |

At least one predicate required before fetch.

## Cost syntax

- Empty → omit
- `3` → `mainCost=3`
- `2,4,6` → repeated `mainCost[]`
- `2-5` → `mainCost[]=2` … `mainCost[]=5`
- Invalid → inline error, no fetch

## Layout

- Left: filter panel
- Right: query preview, status (timing, total), card list placeholder, collapsible JSON

## API follow-up (not in this UI release)

- Multi-slot `effect[N]` parsing and `effectMode` in `uniques-http-api/src/cards.rs`
- `set[]`, power stats, CORS for static hosting

## Run

```bash
# Terminal 1 — API
cargo run -p uniques-http-api

# Terminal 2 — UI
cd demo-ui && npm install && npm run dev
```

Optional: `VITE_API_BASE_URL` in `.env` for non-proxied API host.
