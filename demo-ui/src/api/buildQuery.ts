import { parseCostInput } from './parseCostInput';
import type { BuildQueryResult, EffectSlot, FilterState } from '../types';

function slotHasValues(slot: EffectSlot): boolean {
  return slot.t.trim() !== '' || slot.c.trim() !== '' || slot.o.trim() !== '';
}

export function activeEffectSlotCount(state: FilterState): number {
  return state.effects.filter(slotHasValues).length;
}

function appendCostParams(
  params: URLSearchParams,
  key: string,
  values: number[],
): void {
  if (values.length === 0) {
    return;
  }
  if (values.length === 1) {
    params.set(key, String(values[0]));
    return;
  }
  for (const value of values) {
    params.append(`${key}[]`, String(value));
  }
}

function appendEffectField(
  params: URLSearchParams,
  slotIndex: number,
  field: 't' | 'c' | 'o',
  raw: string,
): void {
  const value = raw.trim();
  if (!value) {
    return;
  }
  params.set(`effect[${slotIndex}][${field}]`, value);
}

export type BuildQueryOptions = {
  cursor?: number;
};

export function buildQuery(
  state: FilterState,
  options?: BuildQueryOptions,
): BuildQueryResult {
  const params = new URLSearchParams();

  let handCostError: string | undefined;
  let reserveCostError: string | undefined;

  // Skip empty UI slots; compact active slots to effect[0], effect[1], …
  let effectIndex = 0;
  for (const slot of state.effects) {
    if (!slotHasValues(slot)) {
      continue;
    }
    appendEffectField(params, effectIndex, 't', slot.t);
    appendEffectField(params, effectIndex, 'c', slot.c);
    appendEffectField(params, effectIndex, 'o', slot.o);
    effectIndex += 1;
  }

  if (activeEffectSlotCount(state) >= 2 && state.effectMode === 'or') {
    params.set('effectMode', 'or');
  }

  if (slotHasValues(state.support)) {
    const supportFields: Array<['t' | 'c' | 'o', string]> = [
      ['t', state.support.t],
      ['c', state.support.c],
      ['o', state.support.o],
    ];
    for (const [field, raw] of supportFields) {
      const value = raw.trim();
      if (value) {
        params.set(`support[${field}]`, value);
      }
    }
  }

  for (const faction of state.factions) {
    params.append('faction[]', faction);
  }

  if (state.handCost.trim()) {
    const parsed = parseCostInput(state.handCost);
    if (!parsed.ok) {
      handCostError = parsed.error;
    } else {
      appendCostParams(params, 'mainCost', parsed.values);
    }
  }

  if (state.reserveCost.trim()) {
    const parsed = parseCostInput(state.reserveCost);
    if (!parsed.ok) {
      reserveCostError = parsed.error;
    } else {
      appendCostParams(params, 'recallCost', parsed.values);
    }
  }

  if (handCostError || reserveCostError) {
    return { ok: false, handCostError, reserveCostError };
  }

  const limitRaw = state.limit.trim();
  const limitParsed =
    limitRaw === '' ? 50 : Number.parseInt(limitRaw, 10);
  const limit = Math.min(
    200,
    Math.max(1, Number.isNaN(limitParsed) ? 50 : limitParsed),
  );
  params.set('limit', String(limit));

  if (options?.cursor !== undefined) {
    params.set('cursor', String(options.cursor));
  }

  return { ok: true, params };
}

export function buildQueryString(
  state: FilterState,
  options?: BuildQueryOptions,
): BuildQueryResult {
  return buildQuery(state, options);
}

export function buildRequestPath(
  state: FilterState,
  options?: BuildQueryOptions,
): BuildQueryResult & {
  path?: string;
  queryString?: string;
} {
  const result = buildQuery(state, options);
  if (!result.ok) {
    return result;
  }
  const qs = result.params.toString();
  return {
    ok: true,
    params: result.params,
    queryString: qs,
    path: qs ? `/api/v2/cards?${qs}` : '/api/v2/cards',
  };
}

export function getApiBaseUrl(): string {
  const base = import.meta.env.VITE_API_BASE_URL?.trim() ?? '';
  return base.replace(/\/$/, '');
}

export function buildFullUrl(
  state: FilterState,
  options?: BuildQueryOptions,
): BuildQueryResult & {
  url?: string;
  queryString?: string;
} {
  const result = buildRequestPath(state, options);
  if (!result.ok || !result.path) {
    return result;
  }
  const apiBase = getApiBaseUrl();
  const url = apiBase ? `${apiBase}${result.path}` : result.path;
  return { ...result, url };
}
