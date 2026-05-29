export type EffectSlot = {
  t: string;
  c: string;
  o: string;
};

/** One row from `GET /api/v2/effects`. */
export type EffectCatalogItem = {
  idGd: number;
  text: Record<string, string>;
  isEcho?: boolean;
  isMain?: boolean;
};

export type EffectsCatalogResponse = {
  triggers: EffectCatalogItem[];
  conditions: EffectCatalogItem[];
  output: EffectCatalogItem[];
};

export type EffectsCatalogStatus = 'loading' | 'ready' | 'error';

export type EffectMode = 'and' | 'or';

export type FilterState = {
  effects: EffectSlot[];
  effectMode: EffectMode;
  support: EffectSlot;
  factions: string[];
  handCost: string;
  reserveCost: string;
  limit: string;
  debugBgaTrigram: boolean;
};

export const FACTIONS = ['AX', 'BR', 'LY', 'MU', 'OR', 'YZ'] as const;

export type FactionCode = (typeof FACTIONS)[number];

export const DEFAULT_FILTER_STATE: FilterState = {
  effects: [{ t: '', c: '', o: '' }],
  effectMode: 'and',
  support: { t: '', c: '', o: '' },
  factions: [],
  handCost: '',
  reserveCost: '',
  limit: '',
  debugBgaTrigram: false,
};

export type CardsIter = {
  total: number;
  cursor?: number;
};

export type CardFaction = {
  code: string;
  name: string;
};

export type CardSet = {
  reference: string;
  name: string;
  code?: string;
};

export type CardSubType = {
  reference: string;
  name: Record<string, string>;
};

export type CardV2 = {
  reference: string;
  name: Record<string, string>;
  artist: string;
  set: CardSet;
  cardSubTypes: CardSubType[];
  mainCost: number;
  recallCost: number;
  forestPower: number;
  mountainPower: number;
  oceanPower: number;
  faction: CardFaction;
  mainEffect: Record<string, string>;
  echoEffect: Record<string, string>;
  debug_bga_trigram?: string;
};

export type CardsResponse = {
  iter: CardsIter;
  cards: CardV2[];
};

export type ApiError = {
  error: string;
};

export type CostParseResult =
  | { ok: true; values: number[] }
  | { ok: false; error: string };

export type BuildQueryResult =
  | { ok: true; params: URLSearchParams }
  | { ok: false; handCostError?: string; reserveCostError?: string };
