## Plan 05: `GET /api/v2/cards` — support `faction[]`, `mainCost*`, `recallCost*`

### Goal

Implement search filters for:
- `faction[]` (and alias `faction=...`)
- `mainCost` / `mainCost[gt|gte|lt|lte]` / `mainCost[]`
- `recallCost` / `recallCost[gt|gte|lt|lte]` / `recallCost[]`

These must be enforced by the query engine (roaring bitmap intersection) and align with the existing index data already loaded into `AppState`:
- `AppState.factions(): BTreeMap<Faction, RoaringBitmap>`
- `AppState.stats(): BTreeMap<StatField, [RoaringBitmap; 16]>`

### API semantics

#### Faction
- Accept:
  - Spec: `faction[]=AX&faction[]=BR` (repeated key)
  - Alias: `faction=AX,BR` (CSV convenience)
- Match semantics:
  - OR within provided factions (union)
  - AND with all other filters (intersection)
- Validation:
  - Unknown faction code => 400

#### Main/Recall cost
- Accept:
  - Exact: `mainCost=3` / `recallCost=2`
  - Range: `mainCost[gt|gte|lt|lte]=N` (same for `recallCost`)
  - Array: `mainCost[]=2&mainCost[]=3` (same for `recallCost`)
  - For array values, also accept CSV in each value: `mainCost[]=2,3`
- Validation:
  - Cost values must be in range `0..=15` (stats index has 16 buckets)
- Combination rules:
  - Do **not** allow mixing exact vs range vs array for the same field
    - Example: `mainCost[]=2&mainCost[lte]=3` => 400
- Match semantics:
  - Exact: select that bucket bitmap
  - Array: union of the listed bucket bitmaps
  - Range: union of all bucket bitmaps that satisfy the comparison
  - Then intersect with other filters

### Implementation outline (files)

#### 1) Parse query params as a multimap
File: `uniques-http-api/src/cards.rs`
- Replace `Query<HashMap<String, String>>` with `RawQuery` so repeated keys can be captured.
- Parse with `url::form_urlencoded::parse(...)` into `HashMap<String, Vec<String>>`.
- Keep existing CSV handling for idGd parameters (allow multiple occurrences per key + CSV in values).

#### 2) Extend request model with faction and cost predicates
File: `uniques-http-api/src/cards.rs`
- Add:
  - `factions: Vec<Faction>`
  - `main_cost: Option<CostPredicate>`
  - `recall_cost: Option<CostPredicate>`
- Implement parsing helpers:
  - `parse_factions(...)` supporting both `faction[]` and `faction` alias
  - `parse_cost_predicate(..., "mainCost")` and `parse_cost_predicate(..., "recallCost")`
  - Reject mixed forms per field

#### 3) Integrate faction/stats into query bitmap
File: `uniques-http-api/src/cards.rs`
- Update bitmap building to AND together:
  - existing ability bitmap(s) (effect/support)
  - faction union bitmap (if factions specified)
  - mainCost bitmap (if specified)
  - recallCost bitmap (if specified)
- Relax predicate requirement:
  - Allow queries that specify only faction and/or cost filters (no longer require ability filters)

### Verification
File: `uniques-http-api/src/cards.rs`
- Add unit tests for:
  - repeated + CSV alias parsing for `faction`
  - invalid faction => 400
  - exact/array/range parsing for costs
  - reject mixing (array + range) for costs
  - reject out-of-range cost values
  - bitmap intersection correctness (ability ∩ faction ∩ cost)

### Dependencies
- Add `url = "2"` to `uniques-http-api/Cargo.toml` for `form_urlencoded` parsing.

