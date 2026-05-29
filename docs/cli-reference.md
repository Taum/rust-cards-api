# CLI reference

This page lists every command-line interface exposed by this repository.

| Tool | How to run | Interface |
|------|------------|-----------|
| **`alt-indexer`** | `cargo run --manifest-path alt-indexer/Cargo.toml -- …` (from repo root), or `cargo run` inside `alt-indexer/` | Subcommands + flags (clap) |
| **`uniques-http-api`** | `cargo run --manifest-path uniques-http-api/Cargo.toml`, or `cargo run` inside `uniques-http-api/` | Environment variables only (no subcommands) |

There is no workspace `Cargo.toml` at the repository root; each crate is built from its own manifest.

For narrative context, see [Architecture overview](architecture.md).

---

## `alt-indexer`

Binary crate in [`alt-indexer/`](../alt-indexer/). Indexes Equinox-style Unique card JSON into Roaring bitmaps, `cards.bin`, and related metadata.

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- --help
cargo run --manifest-path alt-indexer/Cargo.toml -- <SUBCOMMAND> --help
```

Global options: `-h`, `--help` only (no global flags).

### Typical workflow

```bash
# 1. Build one set index
cargo run --manifest-path alt-indexer/Cargo.toml -- build \
  --root /path/to/equinox-cards/cards-unique-COREKS \
  --set COREKS \
  --out ./full_index

# 2. Repeat for other sets, then merge
cargo run --manifest-path alt-indexer/Cargo.toml -- merge \
  --index-dir ./full_index \
  --sets COREKS,CORE,ALIZE \
  --out ./full_index/ALL_SETS

# 3. Inspect
cargo run --manifest-path alt-indexer/Cargo.toml -- query \
  --index-dir ./full_index \
  --set ALL_SETS \
  --id-gd 24,191 \
  --list 10
```

---

### `build`

Crawl a dataset directory and write a per-set index under `<out>/<SET>/`.

| Option | Required | Description |
|--------|----------|-------------|
| `--root <PATH>` | yes | Dataset root containing `json/<SET>/...` card files |
| `--set <CODE>` | yes | Set code (e.g. `COREKS`, `CORE`, `ALIZE`, `BISE`) |
| `--out <PATH>` | yes | Output directory; files go in `<out>/<SET>/` |
| `--limit <N>` | no | Stop after indexing **N** files (testing / partial builds) |
| `--profile` | no | Print phase timings (read, parse, process, write) |

**Environment**

| Variable | Effect |
|----------|--------|
| `ALT_INDEXER_PROFILE=1` or `true` | Same as `--profile` (case-insensitive) |

**Example**

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- build \
  --root "../equinox-cards/cards-unique-COREKS" \
  --set COREKS \
  --out ./full_index \
  --profile
```

On success, prints a one-line summary: output path, file count, family count, idGd bitmap count, and `total_bit_span`.

---

### `decode`

Map a global **`card_index`** (bit position) back to a card reference using `catalog.json`.

| Option | Required | Description |
|--------|----------|-------------|
| `--catalog <PATH>` | yes | Path to `catalog.json` (per-set or merged) |
| `--bit <N>` | yes | `card_index` to decode (`u32`) |

**Example**

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- decode \
  --catalog ./full_index/ALL_SETS/catalog.json \
  --bit 42
```

Prints the reference string and `familyId` / `uniqueID`.

---

### `query`

Query an existing index directory. You must pass **either** `--id-gd` **or** `--refid` (mutually exclusive).

| Option | Required | Description |
|--------|----------|-------------|
| `--index-dir <PATH>` | yes | Parent of set folders (e.g. `./full_index`) |
| `--set <NAME>` | yes | Set or merged folder name (e.g. `COREKS`, `ALL_SETS`) |
| `--id-gd <IDS>` | one of | Comma-separated idGd values (e.g. `24,191,76`) |
| `--refid <REF>` | one of | Single card reference (e.g. `ALT_COREKS_B_AX_04_U_10`) |
| `--list <N>` | no | Decode and print up to **N** matches (idGd mode only) |
| `--show-effect` | no | Print translated effect text instead of a stats table |
| `--locale <LOCALE>` | no | Locale for effect text (default: `en_US`) |
| `--whole-card` | no | Use whole-card bitmaps (`id_gd/<id>.roar`) instead of per-line (`_m1`, `_m2`, `_m3`, `_ec`) |

**`--id-gd` semantics**

1. Each id is looked up in `idgd_catalog.json` and classified as **TRIGGER**, **CONDITION**, or **OUTPUT**.
2. Bitmaps for ids in the same bucket are **unioned**.
3. The final result is **(trigger union) ∩ (condition union) ∩ (output union)**; empty buckets are skipped.
4. Default mode intersects across **per-line** sub-indexes (main lines M1–M3 and echo); `--whole-card` uses combined per-id bitmaps instead.

**`--refid`**

- Looks up one card and prints effect-oriented output (implies `--show-effect`-style layout).
- Cannot be combined with `--id-gd`, `--list`, `--show-effect`, or `--whole-card`.

**Examples**

```bash
# Count cards matching idGd 24 (table output if --list given)
cargo run --manifest-path alt-indexer/Cargo.toml -- query \
  --index-dir ./full_index --set ALL_SETS --id-gd 24

# Multi-idGd query with effect text
cargo run --manifest-path alt-indexer/Cargo.toml -- query \
  --index-dir ./full_index --set ALL_SETS \
  --id-gd 24,191,76 --show-effect --list 5 --locale fr_FR

# Single card by reference
cargo run --manifest-path alt-indexer/Cargo.toml -- query \
  --index-dir ./full_index --set ALL_SETS \
  --refid ALT_COREKS_B_AX_04_U_10
```

---

### `merge`

Merge two or more **existing** per-set indexes into one global index. Output files are written **directly** under `--out` (not `<out>/<SET>/`).

| Option | Required | Description |
|--------|----------|-------------|
| `--index-dir <PATH>` | yes | Directory containing `<SET>/catalog.json` for each source set |
| `--sets <LIST>` | yes | Comma-separated set codes in **precedence order** (overlap grouping and tie-breaking) |
| `--out <PATH>` | yes | Output folder for the merged index (e.g. `./full_index/ALL_SETS`) |

**Example**

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- merge \
  --index-dir ./full_index \
  --sets COREKS,CORE,ALIZE,BISE \
  --out ./full_index/ALL_SETS
```

See [ALL_SETS index format](ALL_SETS-index-format.md) for merge ordering and on-disk layout.

---

### `audit-missing`

Report cards that are allocated in the catalog bit span but missing or invalid in `cards.bin`, focusing on families where `max_unique_id != card_count` (likely gaps).

| Option | Required | Description |
|--------|----------|-------------|
| `--index-dir <PATH>` | yes | Parent of set folders |
| `--set <NAME>` | yes | Set folder to audit |
| `--json` | no | Emit JSON keyed by `ALT_<SET>_B_<family_id>` with missing reference arrays (default: human-readable text) |

**Example**

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- audit-missing \
  --index-dir ./full_index \
  --set COREKS \
  --json
```

---

### `bench-query`

Benchmark random idGd queries against an index. Preloads bitmaps and `cards.bin` into memory so timings exclude per-query disk I/O.

| Option | Required | Default | Description |
|--------|----------|---------|-------------|
| `--index-dir <PATH>` | yes | — | Parent of set folders |
| `--set <NAME>` | yes | — | Set or merged folder name |
| `--queries <N>` | no | `5000` | Number of **timed** queries |
| `--warmup <N>` | no | `200` | Warmup iterations (not recorded) |
| `--multi-ids <MIN-MAX>` | no | — | Multi-id mode: pick random **K** ids per query with `MIN ≤ K ≤ MAX`, bucket as TRIGGER/CONDITION/OUTPUT, then intersect unions (e.g. `6-12`) |
| `--seed <U64>` | no | random | RNG seed; if omitted, a time-based seed is chosen and printed in the report |
| `--json-out <PATH>` | no | — | Write machine-readable benchmark JSON |
| `--print-samples <N>` | no | — | Print first **N** sampled queries (debug; adds I/O noise) |
| `--whole-card` | no | `false` | Use whole-card bitmaps instead of per-line |

**Modes**

- **Single-id** (default): each timed query picks one id from `idgd_catalog.json` and runs the same intersection logic as `query`.
- **Multi-id** (`--multi-ids`): each query picks K random catalog ids, splits into buckets, intersects unions.

Reports latency stats for: **count** (`bitmap.len()`), **first_50** (decode 50 cards from the start of the bitmap), and **offset_10000_50** (skip 10 000 or take last 50, then decode).

**Example**

```bash
cargo run --manifest-path alt-indexer/Cargo.toml -- bench-query \
  --index-dir ./full_index \
  --set ALL_SETS \
  --queries 10000 \
  --multi-ids 6-12 \
  --seed 42 \
  --json-out ./bench.json
```

---

## `uniques-http-api`

HTTP server in [`uniques-http-api/`](../uniques-http-api/). It has **no CLI flags**; configuration is via environment variables (and optional `.env` / `.env.local` in that crate’s directory).

```bash
cargo run --manifest-path uniques-http-api/Cargo.toml
```

### Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `INDEX_PATH` | **yes** | — | Path to the index folder itself (e.g. `../alt-indexer/full_index/ALL_SETS`) |
| `PORT` | no | `8080` | TCP port; bind address is always `0.0.0.0` |

**Local development**

Copy [`uniques-http-api/.env.example`](../uniques-http-api/.env.example) to `.env` and optionally `.env.local` from [`.env.local.template`](../uniques-http-api/.env.local.template). Typical local values:

```env
PORT=8234
INDEX_PATH=../alt-indexer/full_index/ALL_SETS
```

`load_env()` loads `.env` first, then **overrides** with `.env.local`.

**Docker / Cloud Run**

- Image embeds index at `/opt/index/ALL_SETS` by default.
- Cloud Run sets `PORT`; override `INDEX_PATH` if the index is mounted elsewhere.

See [uniques-http-api README](../uniques-http-api/README.md) and [API spec](../uniques-http-api/docs/api-spec.md).

---

## `demo-ui`

The React demo is a separate npm project ([`demo-ui/`](../demo-ui/)); it does not ship a Rust CLI. Configure the API base URL via Vite env (see `demo-ui/README.md` and `demo-ui/.env.example`).
