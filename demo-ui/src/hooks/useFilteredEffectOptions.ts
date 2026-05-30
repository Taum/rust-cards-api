import { useEffect, useMemo, useRef, useState } from 'react';
import {
  buildFilteredEffectsUrl,
  type FilteredEffectsTarget,
} from '../api/buildQuery';
import type { ApiError, EffectsFilteredResponse, FilterState } from '../types';

const DEBOUNCE_MS = 150;

export type FilteredEffectOptionsState = {
  /** Still-possible idGds for the edited box, or `null` when narrowing is off/unknown. */
  ids: number[] | null;
  loading: boolean;
  error: string | null;
};

const IDLE_STATE: FilteredEffectOptionsState = {
  ids: null,
  loading: false,
  error: null,
};

/**
 * Fetch the still-possible idGds for the currently edited effect combobox via
 * `GET /api/v2/effects/filtered`. Debounced and abortable; `enabled=false`
 * short-circuits to `ids: null` (full catalog). On error, falls back to `null`
 * so manual typing keeps working.
 */
export function useFilteredEffectOptions(
  filters: FilterState,
  target: FilteredEffectsTarget | null,
  enabled: boolean,
): FilteredEffectOptionsState {
  const [state, setState] = useState<FilteredEffectOptionsState>(IDLE_STATE);

  const filtersRef = useRef(filters);
  filtersRef.current = filters;
  const targetRef = useRef(target);
  targetRef.current = target;

  const key = useMemo(
    () =>
      enabled && target ? JSON.stringify({ filters, target }) : null,
    [enabled, target, filters],
  );

  useEffect(() => {
    if (key === null) {
      setState(IDLE_STATE);
      return;
    }

    const controller = new AbortController();
    const timer = setTimeout(() => {
      const activeTarget = targetRef.current;
      if (!activeTarget) {
        return;
      }
      const url = buildFilteredEffectsUrl(filtersRef.current, activeTarget);
      setState((prev) => ({ ...prev, loading: true, error: null }));

      void (async () => {
        try {
          const res = await fetch(url, { signal: controller.signal });
          if (!res.ok) {
            let message = `HTTP ${res.status}`;
            try {
              const body = (await res.json()) as ApiError;
              if (body.error) {
                message = body.error;
              }
            } catch {
              // ignore parse errors
            }
            if (controller.signal.aborted) {
              return;
            }
            setState({ ids: null, loading: false, error: message });
            return;
          }

          const body = (await res.json()) as EffectsFilteredResponse;
          if (controller.signal.aborted) {
            return;
          }
          setState({ ids: body.idGds ?? [], loading: false, error: null });
        } catch (err) {
          if (controller.signal.aborted) {
            return;
          }
          const message =
            err instanceof Error ? err.message : 'Failed to narrow effects';
          setState({ ids: null, loading: false, error: message });
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [key]);

  return state;
}
