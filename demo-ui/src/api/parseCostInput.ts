import type { CostParseResult } from '../types';

const MIN_COST = 0;
const MAX_COST = 15;

function parseInteger(token: string): number | null {
  const trimmed = token.trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }
  const value = Number.parseInt(trimmed, 10);
  if (value < MIN_COST || value > MAX_COST) {
    return null;
  }
  return value;
}

function expandToken(token: string): number[] | null {
  const trimmed = token.trim();
  if (!trimmed) {
    return null;
  }

  const rangeMatch = /^(\d+)-(\d+)$/.exec(trimmed);
  if (rangeMatch) {
    const start = Number.parseInt(rangeMatch[1]!, 10);
    const end = Number.parseInt(rangeMatch[2]!, 10);
    if (
      Number.isNaN(start) ||
      Number.isNaN(end) ||
      start > end ||
      start < MIN_COST ||
      end > MAX_COST
    ) {
      return null;
    }
    const values: number[] = [];
    for (let n = start; n <= end; n++) {
      values.push(n);
    }
    return values;
  }

  const single = parseInteger(trimmed);
  return single === null ? null : [single];
}

export function parseCostInput(input: string): CostParseResult {
  const trimmed = input.trim();
  if (!trimmed) {
    return { ok: true, values: [] };
  }

  const tokens = trimmed.split(',');
  const values: number[] = [];

  for (const token of tokens) {
    if (!token.trim()) {
      return { ok: false, error: 'Empty token in cost list' };
    }
    const expanded = expandToken(token);
    if (expanded === null) {
      return {
        ok: false,
        error: `Invalid cost token "${token.trim()}" (use 0–15, N-M range, or comma list)`,
      };
    }
    values.push(...expanded);
  }

  const deduped = [...new Set(values)].sort((a, b) => a - b);
  return { ok: true, values: deduped };
}
