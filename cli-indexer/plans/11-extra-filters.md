# Extra filters on card indexes

## Goal

Register **arbitrary card lists** as first-class filters on an existing index (e.g. `ALL_SETS`): read card references from a file, build a Roaring bitmap, write `extra/<filter-id>.roar`, and append metadata to `extra_catalog.json` for query-time **AND** vs **AND NOT** combination.

Later phases can teach `uniques-http-api` to load and apply these filters; this plan covers **`add-extra-filter`** plus the on-disk layout.

---

## Why inverted (negated) filters matter

Roaring bitmaps store **set bits** (card indices). A filter for “**everything except** a few thousand cards” is huge if stored literally (millions of bits set); it stays small if you store only the **exceptions** (the few thousand excluded cards).

**Convention for this project:**

| `negated` | `.roar` file contains | Default query combine |
|-----------|----------------------|------------------------|
| `false` | Cards **in** the filter (include list) | `result &= filter` |
| `true` | Cards **excluded** from the filter (exception list) | `result &= !filter` within universe (`total_bit_span` from `manifest.json`) |

Implementation at query time: for `negated: true`, apply `result -= (result & filter_bitmap)`, using `manifest.total_bit_span` as the universe bound.

Forward (non-negated) include lists stay sparse and are the common case.

---

## On-disk layout

```text
ALL_SETS/
  catalog.json
  manifest.json
  extra_catalog.json
  extra/
    <id>.roar
  id_gd/
    ...
```

### `extra_catalog.json` schema

```json
{
  "version": 1,
  "set": "ALL_SETS",
  "entries": [
    {
      "id": "exclude-banned",
      "type": "property",
      "negated": true,
      "card_count": 1200,
      "bitmap_bytes": 4500,
      "bitmap_file": "extra/exclude-banned.roar"
    }
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `id` | yes | Stable filter identifier (CLI `--filter-id`); unique among `entries` |
| `type` | no | `"format"` or `"property"` when set via CLI; omitted in JSON if not passed |
| `negated` | yes | `true` → combine with AND NOT; `false` → AND |
| `card_count` | yes | `bitmap.len()` at registration time |
| `bitmap_bytes` | yes | Serialized `.roar` size on disk |
| `bitmap_file` | yes | Relative path under index root, always `extra/<id>.roar` |

---

## CLI: `add-extra-filter`

```bash
alt-indexer add-extra-filter \
  --index-dir ./alt-indexer/full_index/ALL_SETS \
  --filter-id exclude-banned \
  --refs-file ./lists/banned.txt \
  --type property \
  --negated
```

### Parameters

| Flag | Required | Description |
|------|----------|-------------|
| `--index-dir` | yes | Index root containing `catalog.json` and `manifest.json` |
| `--filter-id` | yes | Filter id (slug); becomes `extra/<filter-id>.roar` and `entries[].id` |
| `--refs-file` | yes | Text file: one card reference per line; blank lines and `#` comments ignored |
| `--type` | no | `format` or `property`; omitted in catalog if not set |
| `--negated` | no | Presence flag: if passed, `negated: true`; if omitted, `negated: false` |
| `--replace` | no | Overwrite existing filter (bitmap + catalog entry); default errors on duplicate `--filter-id` |

### Behavior

1. Load `catalog.json` + `manifest.json` from `--index-dir`.
2. Read `--refs-file`; parse refs → `card_index`; dedupe; fail on first unresolvable ref.
3. Build and serialize Roaring bitmap to `extra/<filter-id>.roar`.
4. Append or replace entry in `extra_catalog.json` (create if missing); error on duplicate `--filter-id` unless `--replace`.

---

## Files

- `src/extra_catalog.rs` — schema, load/save
- `src/add_extra_filter.rs` — command logic
- `src/cli.rs` — subcommand wiring
- `tests/add_extra_filter.rs` — integration test

## Out of scope

- `uniques-http-api` loader / query params for extra filter ids
- `merge` propagating `extra/` across set merges
