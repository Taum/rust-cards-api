import type { CardsQueryState } from '../hooks/useCardsQuery';
import type { CardLocale } from '../locale';

import { CardList } from './CardList';

type ResultsPanelProps = {
  query: CardsQueryState;
  locale: CardLocale;
};

export function ResultsPanel({ query, locale }: ResultsPanelProps) {
  const { status, data, error, durationMs, skipped } = query;
  const cardCount = data?.cards.length ?? 0;
  const total = data?.iter.total;

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <div className="shrink-0 rounded-lg border border-slate-700 bg-slate-900/60 px-4 py-3 text-sm">
        {status === 'loading' && (
          <span className="text-sky-300">Loading…</span>
        )}
        {status === 'error' && error && (
          <span className="text-red-400">Error: {error}</span>
        )}
        {status === 'success' && durationMs !== null && (
          <span className="text-emerald-300">
            {durationMs.toFixed(1)} ms
            {total !== undefined && (
              <>
                {' '}
                · <strong>{total.toLocaleString()}</strong> total matches ·{' '}
                {cardCount} card{cardCount === 1 ? '' : 's'} returned
              </>
            )}
          </span>
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

      {status === 'success' && data && (
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain rounded-lg border border-slate-700/60 bg-slate-950/30 p-2">
          <CardList cards={data.cards} locale={locale} />
        </div>
      )}
    </section>
  );
}
