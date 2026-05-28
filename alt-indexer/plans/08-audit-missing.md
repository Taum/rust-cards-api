# Audit missing cards in an index

## Goal

Add `alt-indexer audit-missing` to find cards that are allocated in the catalog bit span but have no compact record in `cards.bin`.

## CLI

```
alt-indexer audit-missing --index-dir <DIR> --set <SET> [--json]
```

Reads `<index-dir>/<SET>/catalog.json` and `<index-dir>/<SET>/cards.bin` (same layout as `query` / `bench-query`).

## Algorithm

1. Load `catalog.json`.
2. Read `cards.bin` into memory once.
3. Ensure `cards.bin` length ≥ `total_bit_span * 32` bytes.
4. Select families where `max_unique_id != card_count` (suspected gaps).
5. For each such family, scan `card_index` in `[start_bit, start_bit + max_unique_id)`:
   - **Present**: `faction_code != 0` (byte 0).
   - **Missing**: `faction_code == 0` and bytes `[1..32)` are all zero → decode reference via `Catalog::decode_bit` and collect.
   - **Corrupt**: `faction_code == 0` but any byte in `[1..32)` is non-zero → print `ERROR` with `card_index` and reference (do not list as missing).

## Output

### Text (default)

Per family with gaps: header + one missing reference per line. ERROR lines go to stderr.

### JSON (`--json`)

Single object on stdout, keys are the reference prefix (same prefix as each listed reference):

```
ALT_<SET>_B_<family_id>
```

Example:

```json
{
  "ALT_BISE_B_AX_54": [
    "ALT_BISE_B_AX_54_U_17",
    "ALT_BISE_B_AX_54_U_991"
  ]
}
```

Use `family.source_set` when present (merged indexes), else `catalog.set`.

## Integration

| File | Role |
|------|------|
| `src/cli.rs` | `AuditMissing` subcommand |
| `src/audit_missing.rs` | Scan + output |
| `src/catalog.rs` | `Catalog::load`, `decode_bit` |
| `src/compact.rs` | `RECORD_SIZE`, `CompactCardView` |

## Tests

Crafted mini index under `tests/audit_missing.rs`: one family with `max_unique_id > card_count`, `cards.bin` with zeroed slots at known indexes, and one corrupt record (faction 0 + nonzero tail) to assert ERROR output.
