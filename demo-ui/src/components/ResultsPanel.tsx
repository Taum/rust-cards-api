import { useEffect, useRef, useState } from 'react';
import type { CardsQueryState } from '../hooks/useCardsQuery';
import type { CardLocale } from '../locale';
import { ResultsScrollContext } from '../context/ResultsScrollContext';

import { CardList } from './CardList';
import { LoadingSpinner } from './LoadingSpinner';

type ResultsPanelProps = {
  query: CardsQueryState;
  locale: CardLocale;
  showDebugTrigram: boolean;
  withFamilies: boolean;
};

export function ResultsPanel({
  query,
  locale,
  showDebugTrigram,
  withFamilies,
}: ResultsPanelProps) {
  const {
    status,
    cards,
    iter,
    error,
    durationMs,
    skipped,
    loadingMore,
    lastPageCount,
    hasMore,
    loadMore,
  } = query;

  const [scrollRoot, setScrollRoot] = useState<HTMLDivElement | null>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);

  const displayed = cards.length;
  const cursor = iter?.cursor;
  const inFlight = status === 'loading' || loadingMore;

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!scrollRoot || !sentinel) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          loadMore();
        }
      },
      { root: scrollRoot, rootMargin: '200px' },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loadMore, hasMore, status, loadingMore, displayed, scrollRoot]);

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <div className="flex shrink-0 items-center gap-2 rounded-lg border border-slate-700 bg-slate-900/60 px-4 py-3 text-sm">
        {inFlight && <LoadingSpinner />}
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
          {status === 'loading' && (
            <span className="text-sky-300">Loading…</span>
          )}
          {status === 'success' && iter !== null && (
            <span className="text-emerald-300">
              {withFamilies ? (
                <>
                  <strong>{displayed.toLocaleString()}</strong>{' '}
                  {displayed === 1 ? 'family' : 'families'} ·{' '}
                  <strong>{iter.total.toLocaleString()}</strong> matching prints
                </>
              ) : (
                <>
                  <strong>{displayed.toLocaleString()}</strong> displayed ·{' '}
                  <strong>{lastPageCount.toLocaleString()}</strong> returned ·{' '}
                  {cursor !== undefined ? (
                    <>
                      cursor <strong>{cursor.toLocaleString()}</strong>
                    </>
                  ) : (
                    <>end of results</>
                  )}
                  {' '}
                  · <strong>{iter.total.toLocaleString()}</strong> total matches
                </>
              )}
              {durationMs !== null && <> · {durationMs.toFixed(1)} ms</>}
            </span>
          )}
          {status === 'error' && error && (
            <span className="text-red-400">Error: {error}</span>
          )}
          {status === 'success' && error && (
            <span className="text-red-400">Error: {error}</span>
          )}
          {status === 'idle' && skipped && (
            <span className="text-slate-500">
              Fix filter validation errors to run a query.
            </span>
          )}
          {status === 'idle' && !skipped && (
            <span className="text-slate-500">Waiting…</span>
          )}
        </div>
      </div>

      {(status === 'success' || (status === 'error' && displayed > 0)) && (
        <ResultsScrollContext.Provider value={scrollRoot}>
          <div
            ref={setScrollRoot}
            className="min-h-0 flex-1 overflow-y-auto overscroll-contain rounded-lg border border-slate-700/60 bg-slate-950/30 p-2"
          >
            <CardList
              cards={cards}
              locale={locale}
              showDebugTrigram={showDebugTrigram}
              withFamilies={withFamilies}
              families={query.families}
            />
            {hasMore && <div ref={sentinelRef} className="h-1" aria-hidden />}
          </div>
        </ResultsScrollContext.Provider>
      )}
    </section>
  );
}
