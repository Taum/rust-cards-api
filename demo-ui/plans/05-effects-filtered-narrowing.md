# Plan 05: Live effect-combobox narrowing via `/api/v2/effects/filtered`

## Goal

Make each effect combobox narrow its suggestions in real time as filters are added. When a
trigger/condition/output box is focused, call the new
[`GET /api/v2/effects/filtered`](../../uniques-http-api/docs/api-spec.md) endpoint with the current
filter state and `editing=<part>:<slot>`, and restrict that box's dropdown to the returned idGds
(the effects still reachable in the reduced search space).

Today the comboboxes always show the full static catalog for their region
([`catalogOptionsForRegion`](../src/api/effectCatalogOptions.ts)); this plan layers dynamic
narrowing on top, keeping the static catalog as the label/text source.

## API recap

`GET /api/v2/effects/filtered?<current filters>&editing=<part>:<slot>` -> `{ editing, idGds }`.

- Client sends its full `/api/v2/cards` filter state (including the edited group).
- `part` = `trigger` | `condition` | `output`; `slot` = a compacted `effect[N]` index or `support`.
- Server excludes the edited group from the search space and returns only the part's still-possible
  idGds (same-line co-occurrence with the group's other boxes). Presence-only, ids-only.

## UX

- Narrow only the **focused** combobox (one in-flight request), debounced ~150ms and abortable.
- While the request is in flight, keep showing the current list (no flicker). On success, intersect
  the region catalog with `idGds`. On error, silently fall back to the full list (manual typing must
  keep working).
- If `idGds` is empty, show a small "No effects possible with current filters" empty state.
- Re-fetch when the filter state changes while the box stays focused (e.g. after the user picks a
  value in a sibling box), so the list keeps narrowing.

## Slot index mapping (important)

[`buildQuery.ts`](../src/api/buildQuery.ts) compacts active slots: empty UI slots are skipped and
active ones are renumbered to `effect[0]`, `effect[1]`, ... The `editing` slot must use that
**compacted** index, not the raw `filters.effects` array index.

- For a main slot at UI index `i`: `slot = number of active (non-empty) slots in effects[0..i)`.
  (If slot `i` itself is empty/new, this is still the index it would occupy; the server treats a
  missing `effect[N]` as a brand-new group and narrows by the remaining filters.)
- For the support group: `slot = "support"`.

## Implementation

| Piece | File | Change |
| ----- | ---- | ------ |
| Types | [`src/types.ts`](../src/types.ts) | Add `EffectPart = 't' \| 'c' \| 'o'`; `EffectsFilteredResponse = { editing: string; idGds: number[] }`. |
| Query builder | [`src/api/buildQuery.ts`](../src/api/buildQuery.ts) | Extract the shared "append filter params" logic (effects/effectMode/support/factions/sets/name/costs) used by `buildQuery`, then add `buildFilteredEffectsQuery(filters, target)` that reuses it, omits `limit`/`cursor`/`debug`/`reference`, computes the compacted slot index, and appends `editing=<part>:<slot>`. On cost-parse error, omit that cost param (best-effort, never blocks narrowing). |
| Fetch hook | `src/hooks/useFilteredEffectOptions.ts` (new) | `useFilteredEffectOptions(filters, target, enabled)` -> `{ ids: number[] \| null; loading: boolean; error: string \| null }`. Debounced + `AbortController`; `enabled=false` short-circuits to `ids: null`. Keyed on serialized `filters` + `target`. |
| Combobox | [`src/components/EffectIdCombobox.tsx`](../src/components/EffectIdCombobox.tsx) | Add optional props `availableIds?: number[] \| null`, `narrowing?: boolean`, `onFocusChange?: (focused: boolean) => void`. When `availableIds` is non-null, intersect `options` by id before the existing token filter; show a subtle "N possible" hint and an empty state when none. Fire `onFocusChange` on focus/blur (selection re-focuses, so it stays active). |
| Slot fields | [`src/components/EffectSlotFields.tsx`](../src/components/EffectSlotFields.tsx) | Accept the full `filters` prop. Track the focused field (`'t'\|'c'\|'o'\|null`). Call `useFilteredEffectOptions` for `{ region, slotIndex, part }` (map region `echo` -> support slot) with `enabled = focusedPart !== null`. Pass `availableIds`/`narrowing` to the focused combobox only. |
| Panel | [`src/components/FilterPanel.tsx`](../src/components/FilterPanel.tsx) | Thread `filters` into each `EffectSlotFields` (main slots and the support group). |

`App.tsx` already owns `filters` and passes it to `FilterPanel`, so no change there beyond what the
panel forwards. Reuse `getApiBaseUrl()` for the request base.

## Edge cases

- Catalog still loading or failed to load: skip narrowing (the box already handles manual entry).
- Multiple values in the edited box (OR within a bucket): the endpoint ignores the edited bucket, so
  suggestions reflect the group's other boxes + global filters — correct.
- `effectMode=or`: server guarantee is best-effort; acceptable for suggestions.
- Rapid focus changes / typing: debounce + abort prevents stale results from overwriting newer ones.

## Manual test

1. Run the API with the `ALL_SETS` index and `npm run dev` in `demo-ui`.
2. Focus a main Trigger box with no other filters: dropdown shows broadly available triggers.
3. Set the Output box in the same slot, refocus Trigger: trigger list shrinks to ones that form a
   real ability with that output.
4. Add a faction/set filter: the focused box's list shrinks further.
5. Pick a combination with no results: focused box shows the empty state; other boxes unaffected.
6. Support group narrows using the echo line only.

## Deferred

- Narrowing all boxes simultaneously (currently focus-only) and caching responses per filter+target.
- Showing counts per option (endpoint is presence-only by design).
