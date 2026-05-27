# Plan: idGd Compact Card Format

## Goal

Define and implement a **fixed-size binary record** per card, capturing only:

- `mainFaction.reference` (as a compact faction tag)
- Numeric stats from `cardElements`:
  - `MAIN_COST`, `RECALL_COST`, `MOUNTAIN_POWER`, `OCEAN_POWER`, `FOREST_POWER`
- `MAIN_EFFECT` idGd structure:
  - Up to 3 `cardEffect` groups, each with up to 3 `idGd`s ordered as `TRIGGER`, `CONDITION`, `OUTPUT`
- `ECHO_EFFECT` idGd structure:
  - Single group of up to 3 `idGd`s ordered as `TRIGGER`, `CONDITION`, `OUTPUT`

Each card is encoded to a **fixed number of bytes** so we can compute offsets as `base + index * RECORD_SIZE` and seek or mmap quickly.

---

## Requirements

### Faction encoding

We care about the **card’s actual faction** first, not just the path:

- Primary source: `mainFaction.reference` in the JSON.
- Fallback: the **path faction** from `ParsedCardPath.faction` if `mainFaction` is missing or invalid.

We map the final faction tag to an integer:

- `AX = 1`
- `BR = 2`
- `LY = 3`
- `MU = 4`
- `OR = 5`
- `YZ = 6`

and store it as a plain `u8` (value `0` reserved for “unknown/invalid or mismatch”).

If `mainFaction.reference` disagrees with the path, we **trust `mainFaction`** but you can later cross-check against the path using the catalog if needed.

### Numeric stats (from `cardElements`)

Fields and constraints:

- `MAIN_COST` value
- `RECALL_COST` value
- `MOUNTAIN_POWER` value
- `OCEAN_POWER` value
- `FOREST_POWER` value

Assumptions:

- Each value is in range **0–15** inclusive.
- We store each stat as a full `u8` in the binary record.
- Missing fields default to 0.

Example expectations:

- `ALT_COREKS_B_AX_06_U_5`:
  - MAIN_COST = 2
  - RECALL_COST = 2
  - MOUNTAIN_POWER = 0
  - OCEAN_POWER = 2
  - FOREST_POWER = 0

- `ALT_COREKS_B_MU_22_U_3140`:
  - MAIN_COST = 7
  - RECALL_COST = 8
  - MOUNTAIN_POWER = 5
  - OCEAN_POWER = 0
  - FOREST_POWER = 5

### MAIN_EFFECT idGd groups

From `cardElements` where `cardElementType.reference == "MAIN_EFFECT"`:

- For that element, we look at each `cardEffectDisplays[i].cardEffect.cardEffectElements[]`.

We want:

- Up to **3 groups**; each group corresponds to one `cardEffect` (one entry in `cardEffectDisplays`).
- Within a group, exactly **three slots** ordered by `type`:
  - 1st slot: `TRIGGER` idGd
  - 2nd slot: `CONDITION` idGd
  - 3rd slot: `OUTPUT` idGd
- If a given type does not exist in that `cardEffect`, encode `0` in that slot.
- If there are fewer than 3 groups, fill missing groups with `(0, 0, 0)`.

We thus represent MAIN_EFFECT as a 3×3 grid: `[(T1,C1,O1); (T2,C2,O2); (T3,C3,O3)]`.

Example expectations:

- `ALT_COREKS_B_AX_06_U_5`:

  ```text
  MAIN_EFFECT = 24, 191, 76 ; 24, 171, 70 ; 0, 0, 0
  ```

- `ALT_COREKS_B_MU_22_U_3140`:

  ```text
  MAIN_EFFECT = 23, 181, 142 ; 20, 191, 146 ; 0, 0, 0
  ```

### ECHO_EFFECT idGd group

From `cardElements` where `cardElementType.reference == "ECHO_EFFECT"`:

- Across its `cardEffectDisplays[*].cardEffect.cardEffectElements[]`, for each of `TRIGGER`, `CONDITION`, `OUTPUT`: take the first `idGd` or 0.
- If `ECHO_EFFECT` is absent entirely, encode `(0, 0, 0)`.

Example expectations:

- `ALT_COREKS_B_AX_06_U_5`:

  ```text
  ECHO_EFFECT = 0, 0, 0
  ```

- `ALT_COREKS_B_MU_22_U_3140`:

  ```text
  ECHO_EFFECT = 192, 191, 245
  ```

---

## Proposed Binary Layout

### High-level structure

One file per set:

```text
<out>/<SET>/cards.bin
```

- **Record i** corresponds to **card_index i** (same as bit index order from the catalog).
- All records have identical size `RECORD_SIZE` bytes.
- File size: `RECORD_SIZE * total_bit_span`.
- Slots with no card (gaps in UniqueID sequences) are written as all-zero bytes.

### Concrete layout (per card record, 32 bytes)

We choose a simple, aligned layout using only `u8` and `u16`, with 2 bytes reserved for future use.

#### Header block — 6 bytes (faction + 5 stats)

```text
Offset  Size  Field        Type
0       1     faction_code u8   (0 = unknown, 1..6 = AX..YZ)
1       1     main_cost    u8   (0–15)
2       1     recall_cost  u8   (0–15)
3       1     mountain_pow u8   (0–15)
4       1     ocean_pow    u8   (0–15)
5       1     forest_pow   u8   (0–15)
```

#### idGd block — 24 bytes (12 × 16 bits)

Fields in order (indices 0–11):

```text
idx  slot
 0   MAIN_EFFECT group 0 TRIGGER
 1   MAIN_EFFECT group 0 CONDITION
 2   MAIN_EFFECT group 0 OUTPUT
 3   MAIN_EFFECT group 1 TRIGGER
 4   MAIN_EFFECT group 1 CONDITION
 5   MAIN_EFFECT group 1 OUTPUT
 6   MAIN_EFFECT group 2 TRIGGER
 7   MAIN_EFFECT group 2 CONDITION
 8   MAIN_EFFECT group 2 OUTPUT
 9   ECHO_EFFECT TRIGGER
10   ECHO_EFFECT CONDITION
11   ECHO_EFFECT OUTPUT
```

Encoding:

- Each value is stored as a `u16` (little-endian), range `0–4095` (0 = “absent”).
- Consecutive in memory: `id[0]`, `id[1]`, …, `id[11]`.

```text
Offset  Size  Field
6       2     id[0]
8       2     id[1]
...     ...   ...
28      2     id[11]
```

#### Reserved block — 2 bytes

```text
Offset  Size  Field
30      2     reserved (zeros for now; keep for future flags/fields)
```

#### Full record

```text
Offset  Bytes  Content
0       6      header: faction + 5 stats (u8)
6       24     12 × idGd as u16 (little-endian)
30      2      reserved / future use
32      -      end of record
```

Fixed **`RECORD_SIZE = 32` bytes**.

Offset formula:

```text
offset = (card_index as usize) * 32
```

#### Size comparison (for intuition)

| Layout          | Stats type | idGd type | Record size | 5.5 M cards  |
|-----------------|------------|-----------|-------------|--------------|
| This plan       | **u8**     | **u16**   | 32 bytes    | ~168 MiB     |

---

## Implementation Plan

### 1. In-memory struct

```rust
pub struct CompactCardFields {
    pub faction_code: u8,           // 0..=6
    pub main_cost: u8,              // 0..=15
    pub recall_cost: u8,            // 0..=15
    pub mountain_power: u8,         // 0..=15
    pub ocean_power: u8,            // 0..=15
    pub forest_power: u8,           // 0..=15
    pub main_effect: [[u16; 3]; 3], // [group][T,C,O], 0..=4095
    pub echo_effect: [u16; 3],      // [T,C,O], 0..=4095
}
```

The in-memory struct mirrors the on-disk layout: `u8` for stats and `u16` for idGd values.

### 2. Parsing

From each card JSON + `ParsedCardPath`:

- **Faction**: map `ParsedCardPath.faction` via `AX=1…YZ=6`, default 0.
- **Stats**: look up each `cardElementType.reference` in `cardElements`, parse `value` as `u8`, default 0.
- **MAIN_EFFECT**: find the `MAIN_EFFECT` element; for each of up to 3 `cardEffectDisplays`, bucket elements by `type`, take the first `idGd` per type or 0.
- **ECHO_EFFECT**: find the `ECHO_EFFECT` element if present; across all displays, take first `idGd` per type or 0. All zeros if absent.

### 3. Binary encoder and writer (`src/compact.rs`)

```rust
pub const RECORD_SIZE: usize = 32;

pub fn encode_record(fields: &CompactCardFields) -> [u8; RECORD_SIZE] {
    let mut buf = [0u8; RECORD_SIZE];

    // Header: 6 bytes
    buf[0] = fields.faction_code;
    buf[1] = fields.main_cost;
    buf[2] = fields.recall_cost;
    buf[3] = fields.mountain_power;
    buf[4] = fields.ocean_power;
    buf[5] = fields.forest_power;

    // idGd block: 12 × u16, little-endian
    let ids: [u16; 12] = [
        fields.main_effect[0][0], fields.main_effect[0][1], fields.main_effect[0][2],
        fields.main_effect[1][0], fields.main_effect[1][1], fields.main_effect[1][2],
        fields.main_effect[2][0], fields.main_effect[2][1], fields.main_effect[2][2],
        fields.echo_effect[0],    fields.echo_effect[1],    fields.echo_effect[2],
    ];

    let mut offset = 6;
    for id in ids {
        let bytes = id.to_le_bytes();
        buf[offset] = bytes[0];
        buf[offset + 1] = bytes[1];
        offset += 2;
    }

    // bytes 30–31 are reserved and remain zero
    buf
}

pub fn write_compact_records(
    path: &Path,
    total_bit_span: u32,
    cards: &[(u32, CompactCardFields)],
) -> Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .create(true).truncate(true).write(true).open(path)?;
    file.set_len((total_bit_span as u64) * RECORD_SIZE as u64)?;
    for (card_index, fields) in cards {
        file.seek(SeekFrom::Start(*card_index as u64 * RECORD_SIZE as u64))?;
        file.write_all(&encode_record(fields))?;
    }
    Ok(())
}
```

Integration into `build`: collect `(card_index, CompactCardFields)` during the main loop; call `write_compact_records` when writing output files.

### 5. Reader API

```rust
pub struct CompactCardView<'a> {
    buf: &'a [u8; RECORD_SIZE],
}

impl<'a> CompactCardView<'a> {
    pub fn from_data(data: &'a [u8], card_index: u32) -> Option<Self> {
        let offset = card_index as usize * RECORD_SIZE;
        data.get(offset..offset + RECORD_SIZE)
            .and_then(|s| s.try_into().ok())
            .map(|buf| Self { buf })
    }

    pub fn faction_code(&self) -> u8    { self.buf[0] }
    pub fn main_cost(&self) -> u8       { self.buf[1] }
    pub fn recall_cost(&self) -> u8     { self.buf[2] }
    pub fn mountain_power(&self) -> u8  { self.buf[3] }
    pub fn ocean_power(&self) -> u8     { self.buf[4] }
    pub fn forest_power(&self) -> u8    { self.buf[5] }

    /// Returns the 16-bit idGd at slot idx (0..11).
    pub fn id_gd(&self, idx: usize) -> u16 {
        let base = 6 + idx * 2;
        u16::from_le_bytes([self.buf[base], self.buf[base + 1]])
    }

    pub fn main_effect_group(&self, group: usize) -> [u16; 3] {
        let base = group * 3;
        [self.id_gd(base), self.id_gd(base + 1), self.id_gd(base + 2)]
    }

    pub fn echo_effect(&self) -> [u16; 3] {
        [self.id_gd(9), self.id_gd(10), self.id_gd(11)]
    }
}
```

Loading options:

- **In-memory** (simple): `let data = std::fs::read("cards.bin")?;`
- **mmap** (large sets): `let mmap = memmap2::MmapOptions::new().map(&file)?;`

Both work identically with `CompactCardView::from_data`.

### 6. Memory viability for 5.5M cards

```text
5.5M cards × 32 bytes = 176,000,000 bytes ≈ 168 MiB
```

- Still very comfortable to keep fully in RAM.
- mmap keeps startup cheap and pages in only what you touch.
- Even three sets simultaneously are well under 1 GiB.

---

## Phases

1. **Phase 0** — Add `CompactCardFields` extractor; unit tests on the 3 `tmp` fixtures against the example values.
2. **Phase 1** — Implement `pack_nibbles`, `pack_12_pair`, `unpack_12_pair`; unit tests (round-trip).
3. **Phase 2** — Implement `encode_record` and `write_compact_records`; integrate into `build`.
4. **Phase 3** — Implement `CompactCardView` reader; add `compact-dump` CLI subcommand.
5. **Phase 4** — Optional mmap integration; benchmark sequential scan vs. random access.

## Related Docs

- [plans/idgd-bitset-indexer.md](idgd-bitset-indexer.md) — card_index / catalog / bit-index scheme
- [docs/card-format.md](../docs/card-format.md) — JSON schema and faction/family terminology
