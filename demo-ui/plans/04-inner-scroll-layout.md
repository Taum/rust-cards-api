# Inner scroll layout

Lock the document to the viewport height and route scrolling into two inner regions: the filter sidebar and the card grid. The page body never scrolls.

## Layout

```
VIEWPORT  (html, body, #root: height 100%, overflow hidden)
|
+-- HEADER ........................... fixed, shrink-0
|
+-- MAIN ............................. flex-1, min-h-0
    |
    +-- FILTER SIDEBAR ............... min-h-0, overflow-y-auto  [scroll]
    |
    +-- RIGHT COLUMN ................. flex col, min-h-0
            |
            +-- QUERY PREVIEW ........ fixed, shrink-0
            |
            +-- RESULTS PANEL .......... flex-1, min-h-0
                    |
                    +-- STATUS BAR ..... fixed, shrink-0
                    |
                    +-- CARD GRID ...... flex-1, min-h-0, overflow-y-auto  [scroll]
```

| Region | Scroll? | CSS (summary) |
|--------|---------|----------------|
| `html`, `body`, `#root` | No | `height: 100%`, `overflow: hidden` |
| Header | No | `shrink-0` |
| Filter sidebar | Yes (inner) | `min-h-0 overflow-y-auto` |
| Query preview | No | `shrink-0` |
| Status bar | No | `shrink-0` |
| Card grid | Yes (inner) | `flex-1 min-h-0 overflow-y-auto` |

## Changes

1. **`src/index.css`** — lock `html`, `body`, `#root` to viewport height.
2. **`src/App.tsx`** — flex column shell (`h-full`, `overflow-hidden`); `main` gets `flex-1 min-h-0`; right column is a flex column.
3. **`src/components/FilterPanel.tsx`** — `min-h-0 overflow-y-auto` on `<aside>`.
4. **`src/components/ResultsPanel.tsx`** — remove Raw JSON view; flex column with scrollable card grid.
5. **`src/components/QueryPreview.tsx`** — `shrink-0` so it stays pinned above results.

## Why `min-h-0`

Flex and grid items default to `min-height: auto`, which prevents shrinking below content size. Without `min-h-0` at each level, inner `overflow-y-auto` never activates and the body scrolls instead.
