import { useEffect, useMemo, useRef, useState } from 'react';
import { buildFullUrl } from '../api/buildQuery';
import type { ApiError, CardsResponse, FilterState } from '../types';

const DEBOUNCE_MS = 300;

export type QueryStatus = 'idle' | 'loading' | 'success' | 'error';

export type CardsQueryState = {
  status: QueryStatus;
  data: CardsResponse | null;
  error: string | null;
  durationMs: number | null;
  fetchedAt: number | null;
  url: string | null;
  queryString: string | null;
  handCostError: string | null;
  reserveCostError: string | null;
  skipped: boolean;
};

const initialState: CardsQueryState = {
  status: 'idle',
  data: null,
  error: null,
  durationMs: null,
  fetchedAt: null,
  url: null,
  queryString: null,
  handCostError: null,
  reserveCostError: null,
  skipped: false,
};

function serializeFilters(state: FilterState): string {
  return JSON.stringify(state);
}

export function useCardsQuery(filters: FilterState): CardsQueryState {
  const [queryState, setQueryState] = useState<CardsQueryState>(initialState);
  const serialized = useMemo(() => serializeFilters(filters), [filters]);
  const abortRef = useRef<AbortController | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    debounceRef.current = setTimeout(() => {
      const parsed = buildFullUrl(filters);

      if (!parsed.ok) {
        abortRef.current?.abort();
        setQueryState({
          ...initialState,
          status: 'idle',
          handCostError: parsed.handCostError ?? null,
          reserveCostError: parsed.reserveCostError ?? null,
          url: null,
          queryString: null,
          skipped: true,
        });
        return;
      }

      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;

      const url = parsed.url!;
      const queryString = parsed.queryString ?? '';

      setQueryState((prev) => ({
        ...prev,
        status: 'loading',
        error: null,
        handCostError: null,
        reserveCostError: null,
        url,
        queryString,
        skipped: false,
      }));

      const start = performance.now();

      void (async () => {
        try {
          const response = await fetch(url, { signal: controller.signal });
          const durationMs = performance.now() - start;

          if (controller.signal.aborted) {
            return;
          }

          if (!response.ok) {
            let message = `HTTP ${response.status}`;
            try {
              const body = (await response.json()) as ApiError;
              if (body.error) {
                message = body.error;
              }
            } catch {
              // ignore JSON parse errors
            }
            setQueryState({
              status: 'error',
              data: null,
              error: message,
              durationMs,
              fetchedAt: Date.now(),
              url,
              queryString,
              handCostError: null,
              reserveCostError: null,
              skipped: false,
            });
            return;
          }

          const data = (await response.json()) as CardsResponse;
          if (controller.signal.aborted) {
            return;
          }

          setQueryState({
            status: 'success',
            data,
            error: null,
            durationMs,
            fetchedAt: Date.now(),
            url,
            queryString,
            handCostError: null,
            reserveCostError: null,
            skipped: false,
          });
        } catch (err) {
          if (controller.signal.aborted) {
            return;
          }
          const durationMs = performance.now() - start;
          const message =
            err instanceof Error ? err.message : 'Request failed';
          setQueryState({
            status: 'error',
            data: null,
            error: message,
            durationMs,
            fetchedAt: Date.now(),
            url,
            queryString,
            handCostError: null,
            reserveCostError: null,
            skipped: false,
          });
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [serialized, filters]);

  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  return queryState;
}
