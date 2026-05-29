import { useState } from 'react';
import type { CardsQueryState } from '../hooks/useCardsQuery';
import { CardListPlaceholder } from './CardListPlaceholder';

type ResultsPanelProps = {
  query: CardsQueryState;
};

export function ResultsPanel({ query }: ResultsPanelProps) {
  const [jsonOpen, setJsonOpen] = useState(false);

  const { status, data, error, durationMs, skipped } = query;
  const cardCount = data?.cards.length ?? 0;
  const total = data?.iter.total;

  return (
    <section className="space-y-4">
      <div className="rounded-lg border border-slate-700 bg-slate-900/60 px-4 py-3 text-sm">
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
            Select at least one filter to run a query.
          </span>
        )}
        {status === 'idle' && !skipped && (
          <span className="text-slate-500">Waiting for filters…</span>
        )}
      </div>

      {status === 'success' && data && (
        <>
          <div>
            <h2 className="mb-2 text-sm font-semibold text-slate-200">Cards</h2>
            <CardListPlaceholder cards={data.cards} />
          </div>

          <div>
            <button
              type="button"
              onClick={() => setJsonOpen((open) => !open)}
              className="text-sm text-slate-400 hover:text-slate-200"
            >
              {jsonOpen ? '▼' : '▶'} Raw JSON
            </button>
            {jsonOpen && (
              <pre className="mt-2 max-h-96 overflow-auto rounded-lg border border-slate-700 bg-slate-950 p-3 font-mono text-xs text-slate-300">
                {JSON.stringify(data, null, 2)}
              </pre>
            )}
          </div>
        </>
      )}
    </section>
  );
}
