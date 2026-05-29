export type EffectSlot = {
  t: string;
  c: string;
  o: string;
};

export type EffectMode = 'and' | 'or';

export type FilterState = {
  effects: EffectSlot[];
  effectMode: EffectMode;
  support: EffectSlot;
  factions: string[];
  handCost: string;
  reserveCost: string;
  limit: string;
  cursor: string;
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
  cursor: '',
};

export type CardsIter = {
  total: number;
  cursor?: number;
};

export type CardFaction = {
  code: string;
};

export type CardSet = {
  reference?: string;
  code?: string;
};

export type CardV2 = {
  reference: string;
  mainCost: number;
  recallCost: number;
  forestPower: number;
  mountainPower: number;
  oceanPower: number;
  faction: CardFaction;
  set?: CardSet;
  mainEffect: Record<string, string>;
  echoEffect: Record<string, string>;
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
