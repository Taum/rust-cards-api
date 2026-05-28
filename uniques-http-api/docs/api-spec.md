# AI Query Analysis

## `GET /api/cards` Parameters

### Core filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| [] | `set[]` | repeated array | `set[]=CORE` | Filter by one or more set references. |
| [] | `faction[]` | repeated array | `faction[]=AX` | Filter by one or more faction codes. |

### Numeric and stat filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| [] | `mainCost` | exact integer | `mainCost=3` | Exact main cost. |
| [] | `mainCost[gt]` / `mainCost[gte]` / `mainCost[lt]` / `mainCost[lte]` | ranged integer | `mainCost[gte]=3` | Main cost greater/less than comparisons. |
| [] | `mainCost[]` | repeated array | `mainCost[]=2&mainCost[]=3` | Match any of several exact values. |
| [] | `recallCost` / `recallCost[...]` / `recallCost[]` | integer or ranged/array | `recallCost[lte]=1` | Recall cost filter. |
| [] | `oceanPower` / `oceanPower[...]` / `oceanPower[]` | integer or ranged/array | `oceanPower[]=0&oceanPower[]=1` | Ocean power filter. |
| [] | `mountainPower` / `mountainPower[...]` / `mountainPower[]` | integer or ranged/array | `mountainPower[gt]=5` | Mountain power filter. |
| [] | `forestPower` / `forestPower[...]` / `forestPower[]` | integer or ranged/array | `forestPower=2` | Forest power filter. |

### Effect filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| [] | `effect[0][t]` | integer id or comma-separated list | `5`, `1,5,12` | Ability trigger idGd for slot 0. |
| [] | `effect[0][c]` | integer id or comma-separated list | `3`, `3,16,199` | Ability condition idGd for slot 0. |
| [] | `effect[0][o]` | integer id or comma-separated list | `42`, `42,94,601` | Ability output idGd for slot 0. |
| [] | `effect[1][t]` | integer id or comma-separated list | `24` | Ability trigger idGd for slot 1. |
| [] | `effect[1][c]` | integer id or comma-separated list | `191` | Ability condition idGd for slot 1. |
| [] | `effect[1][o]` | integer id or comma-separated list | `90` | Ability output idGd for slot 1. |
| [] | `effectMode` | enum | `and`, `or` |
| [] | `support[t]` | integer id or comma-separated list | `support[t]=24` | Ability trigger idGd for support effect. |
| [] | `support[c]` | integer id or comma-separated list | `support[c]=191` | Ability condition idGd for support effect. |
| [] | `support[o]` | integer id or comma-separated list | `support[o]=90` | Ability output idGd for support effect. |

If multiple values are specified for a trigger, condition or output, a predicate is created that matches ANY of these values (OR).
If multiple effects are specified, the `effectMode` determines if the card must match at least one of them (`or`) or all of them (`and`).

## `GET /api/cards` Response

The response is a JSON files which includes the total number of matches, a page of results and the cursor for getting the next page of results.

```
{
  iter: {
    total: 51513,
    cursor: 10516,
  },
  cards: [
    {
      reference: "ALT_COREKS_AX_05_U_161",
      mainCost: 2,
      recallCost: 3,
      forestPower: 1,
      mountainPower: 6,
      oceanPower: 3,
      faction: {
        code: "AX"
      }
      mainEffect: {
        en_US: "{R} If there's a card in your Landmarks: Target Character in play or in Reserve gains 2 boosts.  When I leave the Expedition zone — []You may [Augment] target card in play or in Reserve.",
        fr_FR: "{R} S'il y a au moins une carte dans vos Repères : Un Personnage ciblé en jeu ou en Réserve gagne 2 boosts.  Lorsque je quitte la zone d'Expédition — [] Vous pouvez [AUGMENT] une carte ciblée en jeu ou en Réserve."
        de_DE: "...",
        es_ES: "...",
        it_IT: "..."
      },
      echoEffect: {
        en_US: "{D} : []Pay {1} less for the next card you play this turn.",
        fr_FR: "{D} : [] Payez {1} de moins pour la prochaine carte que vous jouez ce tour-ci.",
        de_DE: "...",
        es_ES: "...",
        it_IT: "..."
      }
    },
    {
      reference: "ALT_COREKS_BR_51_U_3467"
      ...
    },
  ]
}
```