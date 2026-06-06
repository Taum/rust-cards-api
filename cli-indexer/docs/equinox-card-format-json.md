# Card File Format

This project indexes card records stored as one JSON object per file.

## Dataset Layout

Observed dataset roots:

```text
../cards-unique-COREKS
../cards-unique-ALIZE
../cards-unique-BISE
```

Observed generic path pattern:

```text
json/<SET>/<faction>/<familyNumber>/ALT_<SET>_B_<faction>_<familyNumber>_U_<uniqueId>.json
```

Shared special-case foiler path (exists on disk but **ignored by the indexer**):

```text
json/<SET>/NE/00/ALT_<SET>_B_NE_FOILER_U.json
```

Representative examples:

- `json/COREKS/AX/06/ALT_COREKS_B_AX_06_U_5.json`
- `json/ALIZE/AX/32/ALT_ALIZE_B_AX_32_U_1.json`
- `json/BISE/AX/49/ALT_BISE_B_AX_49_U_1.json`
Observed folder characteristics:

- Files are nested, not flat.
- The first meaningful level under `json/<SET>` is usually a two-letter faction code such as `AX`, `BR`, `LY`, `MU`, `OR`, or `YZ`.
- The next level is usually a two-digit `familyNumber` such as `06`, `32`, or `49`.
- Each JSON file appears to hold exactly one card record.
### Family Identity

For ordinary card files, the path encodes an important identifier:

- `faction`: a two-letter code, one of `AX`, `BR`, `LY`, `MU`, `OR`, or `YZ`
- `familyNumber`: the two-digit family number within that faction
- `familyId`: the combination of `faction` and `familyNumber`
- **UniqueID**: the unpadded decimal after `_U_` in the filename (e.g. `5` in `..._U_5`)

**UniqueID** is unique within a `familyId`, but not across the project—the same number can appear in different families or sets. The indexer uses `familyId` + UniqueID (with set and product code) to form the full card `reference`.

`familyId` should be treated as a first-class identifier by the indexer. It identifies the card family; UniqueID identifies which unique print within that family.

The number of cards per `familyId` is **variable** (not a fixed count per family). See [plans/idgd-bitset-indexer.md](../plans/idgd-bitset-indexer.md) for how the indexer records `maxUniqueID` per family for reverse lookup.

### Numbering Differences Between Sets

The directory structure is consistent across sets, but the observed `familyNumber` ranges differ:

- `COREKS`: `00`, `04`-`24`
- `ALIZE`: `00`, `31`-`41`, `44`, `46`
- `BISE`: `00`, `49`-`59`

Important distinctions:

- The two-digit folder and filename segment is the `familyNumber`, not the same thing as the collector number.
- The filename mirrors the folder code exactly for ordinary files, for example `AX/32` pairs with `_B_AX_32_`.
- The UniqueID after `_U_` is an unpadded decimal such as `1`, `10`, `100`, `1000`, or `10000`.
- `collectorNumberFormatted` uses a different set-branded namespace, for example `BTG-*`, `TBF-*`, and `WFM-*`.

Because of this, the indexer should store `faction`, `familyNumber`, `familyId`, and `collectorNumberFormatted` as separate fields.

## Top-Level JSON Shape

Across sampled files from `COREKS`, `ALIZE`, and `BISE`, each file contains a single JSON object with these top-level fields:

- `cardType`
- `cardSubTypes`
- `cardSet`
- `rarity`
- `cardElements`
- `isPublic`
- `serializedNumber`
- `cardProduct`
- `illustrator`
- `imagePath`
- `assets`
- `lowerPrice`
- `qrUrlDetail`
- `isExclusive`
- `isOwnerless`
- `reference`
- `translations`
- `id`
- `mainFaction`
- `allImagePath`
- `name`
- `elements`
- `collectorNumberFormatted`
- `isSuspended`
- `isErrated`
- `isBanned`
- `isSerialized`
- `isParentSerialized`

## Important Nested Structures

### `cardType`

Metadata about the broad kind of card.

- `reference`
- `translations`
- `id`
- `name`

`cardType.translations` is a locale map such as `en_US`, `fr_FR`, `de_DE`, `es_ES`, and `it_IT`. Each locale object contains:

- `locale`
- `name`

### `cardSubTypes`

An array of subtype objects:

- `reference`
- `id`
- `name`

Sample subtype counts vary from one to two entries.

### `cardSet`, `rarity`

Small reference objects with:

- `id`
- `reference`
- `name`

### `cardElements`

This is the richest part of the schema and is likely the best source for detailed indexing.

Each array item contains:

- `cardElementType`
- `cardEffectDisplays`
- `value`
- `id`

`cardElementType` contains:

- `reference`
- `id`

Observed `cardElementType.reference` values include:

- `MAIN_COST`
- `RECALL_COST`
- `MOUNTAIN_POWER`
- `OCEAN_POWER`
- `FOREST_POWER`
- `MAIN_EFFECT`
- `ECHO_EFFECT` (optional)

`cardEffectDisplays` is an array. Each item contains:

- `cardElement`
- `cardEffect`
- `sequence`
- `id`

`cardEffect` contains:

- `cardEffectElements`
- `reference`

Each `cardEffectElements` item contains:

- `idGd`
- `cardEffectElementDisplays`
- `type`
- `translations`
- `text`

Observed `type` values include:

- `TRIGGER`
- `CONDITION`
- `OUTPUT`

`cardEffectElementDisplays` is also an array. Each item contains:

- `cardKeyword`
- `sequence`
- `isDescriptionDisplayed`
- `id`

When present, `cardKeyword` contains:

- `translations`
- `id`
- `reference`

### `translations`

The root-level `translations` field is another locale map. Each locale entry contains:

- `name`
- `image`
- `locale`

### `mainFaction`

Faction metadata:

- `reference`
- `color`
- `id`
- `name`

### `allImagePath`

A smaller locale-to-URL map. Observed keys include `en-us` and `fr-fr`.

### `elements`

This is a flattened lookup map for common values already derived from `cardElements`.

Observed keys include:

- `MAIN_COST`
- `RECALL_COST`
- `MOUNTAIN_POWER`
- `OCEAN_POWER`
- `FOREST_POWER`
- `MAIN_EFFECT`
- `ECHO_EFFECT` (optional)

For indexing, `elements` is convenient for quick access, while `cardElements` preserves the richer effect structure.

## Types and Optional Fields

Observed value patterns:

- IDs, references, names, and URLs are strings.
- Several numeric-looking stats are stored as strings, for example `"2"` and `"7"`.
- Boolean flags are used for publication and rules status fields.
- `lowerPrice` is numeric.
- `serializedNumber` is nullable and was `null` in all sampled files.
- `cardEffectDisplays` and `cardEffectElementDisplays` may be empty arrays.
- `ECHO_EFFECT` is optional and not present on every card.
- Files may be either minified or pretty-printed; formatting differences do not appear to reflect schema differences.

## Notable Variations in Sampled Files

Across the three examples:

- `isPublic` can be either `true` or `false`.
- Cards can have different subtype counts.
- `ECHO_EFFECT` may be present or absent.
- The effect text structure varies substantially inside `cardElements`.

Important cautions:

- In one sample, the file path bucket was `OR`, but `mainFaction.reference` was `YZ`.
- `familyNumber` is not the same namespace as `collectorNumberFormatted`.

This means the indexer should not assume that every code embedded in the path or filename is identical to the card's internal faction metadata without validation.

## Indexing Guidance

A practical first-pass index could capture:

- file path
- set code
- faction from the path
- family number from the path
- family identifier composed from faction and family number
- UniqueID from the filename
- card `reference`
- `id`
- `name`
- `cardType.reference`
- subtype references
- `rarity.reference`
- `mainFaction.reference`
- `collectorNumberFormatted`
- locale names from `translations`
- flattened stats/effects from `elements`
- full effect structure from `cardElements`

For search and filtering, `elements` is likely the easiest normalized access path. For advanced text or rules analysis, keep the full `cardElements` payload available as well.
