# Plan 06-1: Lazy loading for Altered card renderer

## Problem

`AlteredCardSlot` calls `window.AlteredRender.mountFromApi` on mount for every result card. With
**Group by family** (`withFamilies`), the API can return on the order of **400** families at once —
each with a full `CardV2` in `cards[]`. Mounting every canvas in one pass caused noticeable lag and
main-thread jank.

Normal paged search is less severe (default `limit` 50) but still benefits from lazy mount when
users raise the limit.

## Approach

1. **Lazy mount** — only call `mountFromApi` when the card slot is near the viewport.
2. **Unmount off-screen** — clear the imperative mount node when the slot leaves view so canvases
   are not kept alive for the whole grid.
3. **Concurrency cap** — at most **4** simultaneous mounts globally (tunable constant).
4. **Placeholders** — preserve layout with the same `aspect-[744/1039]` shell as a rendered card.

**Status:** implemented.

## Implementation

### Mount queue

[`src/api/alteredMountQueue.ts`](../src/api/alteredMountQueue.ts)

- `ALTERED_MOUNT_CONCURRENCY = 4` — raise if scrolling feels slow; lower if the UI still stutters.
- `withAlteredMountSlot(fn)` — acquire/release a slot around each mount.

### Scroll root for `IntersectionObserver`

[`src/context/ResultsScrollContext.tsx`](../src/context/ResultsScrollContext.tsx)

- The results panel scroll container is passed as `root` (not the window), so only cards inside the
  results column are considered visible.
- `rootMargin: 200px` prefetches slightly before enter.

[`ResultsPanel.tsx`](../src/components/ResultsPanel.tsx) sets `scrollRoot` via callback ref and
wraps `CardList` in the provider.

### `AlteredCardSlot` DOM layout (important)

React and `mountFromApi` must **not** share the same DOM node.

```
┌─ shell (aspect-[744/1039], relative) ─────────────┐
│  mountRef (empty, absolute inset-0)  ← renderer │
│  placeholder overlay (z-10, React)               │
│  error overlay (z-20, React)                     │
└──────────────────────────────────────────────────┘
```

- **`mountRef`** — imperative-only; `replaceChildren()` on cleanup. Never put React children here.
- **Placeholder / error** — sibling overlays; hidden when mounted or not visible.

An earlier version put the placeholder inside `mountRef`; `mountFromApi` replaced that subtree and
React later threw `NotFoundError: removeChild`, blanking the page. The split layout fixes that.

### Visibility lifecycle

| State | Behavior |
| --- | --- |
| Off-screen | Empty shell keeps grid size; no mount. |
| Visible, queued | Slate placeholder overlay. |
| Visible, mounting | Up to 4 mounts in flight globally. |
| Mounted | Placeholder removed; canvas in `mountRef`. |
| Scroll away | Cancel in-flight work, `replaceChildren()`, reset state. |

## Files

| File | Role |
| --- | --- |
| [`src/api/alteredMountQueue.ts`](../src/api/alteredMountQueue.ts) | Concurrency limit |
| [`src/context/ResultsScrollContext.tsx`](../src/context/ResultsScrollContext.tsx) | Scroll `root` for observer |
| [`src/components/AlteredCardSlot.tsx`](../src/components/AlteredCardSlot.tsx) | Lazy mount + placeholders |
| [`src/components/ResultsPanel.tsx`](../src/components/ResultsPanel.tsx) | Provides scroll context |

## Related (same demo session, not this plan)

- **`withFamilies`** — [`12-with-families`](../../uniques-http-api/plans/12-with-families.md) on the
  API; demo checkbox, family captions, `cards[]` = one card per family. Lazy loading was added
  primarily to make that path usable.

## Possible follow-ups

- Virtualized grid if card counts grow beyond family mode.
- `requestAnimationFrame` batching for `setMounted` updates (only if profiling shows benefit).
- Reuse canvas / renderer unmount API if Altered exposes one later (today: `replaceChildren()` only).
