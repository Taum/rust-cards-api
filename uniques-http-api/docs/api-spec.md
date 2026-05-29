# AI Query Analysis

## `GET /api/v2/cards` Parameters

### Core filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| Yes | `set[]` | repeated array | `set[]=CORE` | Filter by one or more source set codes (e.g. `CORE`, `COREKS`). Alias: `set=CORE,COREKS` (CSV). OR within listed sets; AND with other filters. |
| Yes | `faction[]` | repeated array | `faction[]=AX` | Filter by one or more faction codes. |

### Numeric and stat filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| Yes | `mainCost` | exact integer | `mainCost=3` | Exact main cost. |
| Yes | `mainCost[gt]` / `mainCost[gte]` / `mainCost[lt]` / `mainCost[lte]` | ranged integer | `mainCost[gte]=3` | Main cost greater/less than comparisons. |
| Yes | `mainCost[]` | repeated array | `mainCost[]=2&mainCost[]=3` | Match any of several exact values. |
| Yes | `recallCost` / `recallCost[...]` / `recallCost[]` | integer or ranged/array | `recallCost[lte]=1` | Recall cost filter. |
| No  | `oceanPower` / `oceanPower[...]` / `oceanPower[]` | integer or ranged/array | `oceanPower[]=0&oceanPower[]=1` | Ocean power filter. |
| No  | `mountainPower` / `mountainPower[...]` / `mountainPower[]` | integer or ranged/array | `mountainPower[gt]=5` | Mountain power filter. |
| No  | `forestPower` / `forestPower[...]` / `forestPower[]` | integer or ranged/array | `forestPower=2` | Forest power filter. |

### Effect filters

| Supported | Parameter | Type / Encoding | Example | Meaning |
|---|---|---|---|---|
| Yes | `effect[0][t]` | integer id or comma-separated list | `5`, `1,5,12` | Ability trigger idGd for slot 0. |
| Yes | `effect[0][c]` | integer id or comma-separated list | `3`, `3,16,199` | Ability condition idGd for slot 0. |
| Yes | `effect[0][o]` | integer id or comma-separated list | `42`, `42,94,601` | Ability output idGd for slot 0. |
| Yes | `effect[1][t]` | integer id or comma-separated list | `24` | Ability trigger idGd for slot 1. |
| Yes | `effect[1][c]` | integer id or comma-separated list | `191` | Ability condition idGd for slot 1. |
| Yes | `effect[1][o]` | integer id or comma-separated list | `90` | Ability output idGd for slot 1. |
| Yes | `effectMode` | enum | `and`, `or` |
| Yes | `support[t]` | integer id or comma-separated list | `support[t]=24` | Ability trigger idGd for support effect. |
| Yes | `support[c]` | integer id or comma-separated list | `support[c]=191` | Ability condition idGd for support effect. |
| Yes | `support[o]` | integer id or comma-separated list | `support[o]=90` | Ability output idGd for support effect. |

If multiple values are specified for a trigger, condition or output, a predicate is created that matches ANY of these values (OR).
If multiple effects are specified, the `effectMode` determines if the card must match at least one of them (`or`) or all of them (`and`).

## `GET /api/v2/cards` Response

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
      name: {
        en_US: "Ayxas, Repented Tyrant",
        fr_FR: "Ayxas, Tyran Repenti"
        ...
      },
      artist: "Artist Name",
      set: {
        reference: "COREKS",
        name: "Beyond the Gates - KS Edition",
        code: "BTG"
      },
      cardSubTypes: [
        {
          reference: "NOBLE",
          name: { en_US: "Noble", fr_FR: "Noble", ... }
        }
      ],
      mainCost: 2,
      recallCost: 3,
      forestPower: 1,
      mountainPower: 6,
      oceanPower: 3,
      faction: {
        code: "AX",
        name: "Axiom"
      },
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


## GET `/api/v2/effects` Response

This endpoint takes no parameters.

It returns a list of the Effect parts available for filtering.

```
{
  triggers: [
    {
      idGd: 1,
      text: {
        en_US: "{R}",
        fr_FR: "{R}",
        de_DE: ...
        es_ES: ...
        it_IT: ...
      },
      isEcho: false,
      isMain: true,
    },
    {
      idGd: 2,
      text: {
        en_US: "When an opponent draws one or more cards or does [RESUPPLY_T] —",
        fr_FR: "Lorsqu'un adversaire pioche au moins une carte ou [RESUPPLY_T] —",
        de_DE: ...
        es_ES: ...
        it_IT: ...
      }
    },
    ...
  ],
  conditions: [
    {
      idGd: 166,
      text: {
        en_US: "If you control two or more Plants other than me:",
        fr_FR: ...
        de_DE: ...
        es_ES: ...
        it_IT: ...
      },
      isEcho: false,
      isMain: true,
    },
    {
      idGd: 167,
      text: {
        en_US: "If there are three or more base statistics of 0 among Characters you control:",
        fr_FR: ...
        de_DE: ...
        es_ES: ...
        it_IT: ...
      }
    },
    ...
  ],
  output: [
    {
      idGd: 193,
      text: {
        en_US: "[AFTER_YOU].",
        fr_FR: ...
        de_DE: ...
        es_ES: ...
        it_IT: ...
      },
      isEcho: false,
      isMain: true,
    },
    ...
  ]
}
```

