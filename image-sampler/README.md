# image-sampler

CLI to sample unique-card images from the merged index, resolve image URLs from equinox JSON, and download via the altered.gg proxy.

Run all commands from the **repository root** (`rust-cards-api/`).

## Prerequisites

- Merged index at `build/full_index/ALL_SETS` (see root `README.md` / `AGENTS.md`).
- Equinox card JSON tree, e.g. `C:\Users\taumx\Documents\GitHub\equinox-cards` with paths like `cards-unique-<SET>/json/<SET>/<faction>/<family>/<ref>.json`.

Use `--release` for long runs (`sample` at 200K, full `download`).

## Pipeline

### 1. Analyze (optional)

Count ability combinations from `cards.bin` — no JSON crawl.

```powershell
cargo run -p image-sampler --release -- analyze `
  --index-dir build/full_index `
  --set ALL_SETS `
  --out-json out/analyze.json
```

### 2. Sample

Build `out/plan.jsonl` (one row per card). Every card gets `en_US`; ~10% also get `fr_FR`; ~1% get all five locales. Default English budget is **200,000** unique strict-tuple cards.

```powershell
cargo run -p image-sampler --release -- sample `
  --index-dir build/full_index `
  --set ALL_SETS `
  --equinox-root "C:\Users\taumx\Documents\GitHub\equinox-cards" `
  --budget 200000 `
  --full-locale-fraction 0.01 `
  --fr-locale-fraction 0.10 `
  --seed 42 `
  --out out/plan.jsonl `
  --out-summary out/plan-summary.json
```

Smaller test run:

```powershell
cargo run -p image-sampler --release -- sample `
  --equinox-root "C:\Users\taumx\Documents\GitHub\equinox-cards" `
  --budget 5000 `
  --out out/plan-test.jsonl `
  --out-summary out/plan-test-summary.json
```

### 3. Resolve URLs

Read only the sampled cards' JSON files; write `out/plan-resolved.jsonl` (one row per card, `locales` map of locale → `Art/...` rel_path). Prod/proxy URLs are rebuilt at download time from hard-coded hosts in the crate.

```powershell
cargo run -p image-sampler --release -- resolve-urls `
  --plan out/plan.jsonl `
  --equinox-root "C:\Users\taumx\Documents\GitHub\equinox-cards" `
  --out out/plan-resolved.jsonl `
  --out-errors out/resolve-errors.jsonl
```

### 4. Download

Fetch images into `out/images/<SET>/<faction>/<family>/<ref>/<locale>.jpg`.

**Resumable:** Re-run the same command after an interrupt (Ctrl+C). Any non-empty `.jpg` already on disk is skipped. `index.jsonl` is appended to (deduped by `(ref, locale)`). Incomplete `.jpg.part` temp files from a killed write are ignored and the image is re-fetched.

```powershell
cargo run -p image-sampler --release -- download `
  --plan out/plan-resolved.jsonl `
  --out-dir out `
  --concurrency 4 `
  --images-per-second 2 `
  --spot-check-n 5 `
  --seed 42
```

`--images-per-second` throttles HTTP fetches globally (default **2**). Skips of files already on disk are not throttled. Use `0` for unlimited.

Resume after interruption (same flags, same `--out-dir`):

```powershell
cargo run -p image-sampler --release -- download `
  --plan out/plan-resolved.jsonl `
  --out-dir out `
  --concurrency 4 `
  --images-per-second 2 `
  --spot-check-n 0 `
  --seed 42
```

Use `--spot-check-n 0` on resume to skip the upfront URL probe when you already validated the pattern. Use `--force` to re-download everything and reset `index.jsonl` / `errors.jsonl`.

## Output layout

```
out/
  plan.jsonl              # sampled cards + locale tiers
  plan-summary.json
  plan-resolved.jsonl     # per-card locale rel_paths
  resolve-errors.jsonl
  images/
    <SET>/<faction>/<family>/<ref>/<locale>.jpg
  index.jsonl             # one row per downloaded/skipped image
  errors.jsonl
  manifest.json           # last download run summary
```

## Locale tiers (defaults)

| Tier | Locales | Share of cards |
|------|---------|----------------|
| `en_only` | en_US | ~90% |
| `en_fr` | en_US + fr_FR | ~9% |
| `full` | en_US + fr_FR + de_DE + es_ES + it_IT | ~1% |

Shape-floor picks (phase 1) use the `full` tier when a card has all five locale images in JSON.
