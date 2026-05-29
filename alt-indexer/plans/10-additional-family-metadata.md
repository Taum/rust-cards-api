# 10 — Additional family metadata

## Goal

Extract per-family card metadata from source JSON into `catalog.json`, then return it on each card from `GET /api/v2/cards`.

Metadata is constant within a family (same art / rules text). Capture it from the **first card** in each family only.

## Catalog fields (`FamilyEntry`)

| Field | Source |
|-------|--------|
| `name` | Top-level `name` + `translations.{locale}.name` |
| `artist` | `illustrator.nickName` |
| `card_sub_types` | `cardSubTypes[]` — `reference` + localized `name` |
| `set` | `cardSet.reference`, `cardSet.name`, `code` from hardcoded map |

Always written on new builds. Old catalogs without these fields are not supported — regenerate indexes.

## Hardcoded lookups

### Set `code` (`cardSet.reference` → code)

| reference | code |
|-----------|------|
| COREKS, CORE | BTG |
| ALIZE | TBF |
| BISE | WFM |
| CYCLONE | SKY |
| DUSTER | SDU |
| EOLE | ROC |

### Faction `name` (API only)

| code | name |
|------|------|
| AX | Axiom |
| BR | Bravos |
| LY | Lyra |
| MU | Muna |
| OR | Ordis |
| YZ | Yzmir |

## API (`CardV2`)

Adds: `name`, `artist`, `set`, `cardSubTypes`, and `faction.name` (alongside existing `faction.code`).

Locale keys: `en_US`, `fr_FR`, `de_DE`, `es_ES`, `it_IT`.

No numeric subtype ids. No consistency check across cards in a family.

## Implementation

- `card.rs` — extend `CardJson`, extraction helpers
- `set_code.rs`, `faction_display.rs` — lookups
- `catalog.rs` — `FamilyEntry` fields, `CatalogBuilder::on_card(parsed, card)`, `family_for_bit`
- `build.rs` — load JSON before catalog registration
- `uniques-http-api/src/cards.rs` — enrich `CardV2` from `family_for_bit`

## Rebuild

Regenerate all set indexes and `ALL_SETS` merge after deploying.
