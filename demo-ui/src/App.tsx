import { useState } from 'react';
import { FilterPanel } from './components/FilterPanel';
import { QueryPreview } from './components/QueryPreview';
import { ResultsPanel } from './components/ResultsPanel';
import { useCardsQuery } from './hooks/useCardsQuery';
import { DEFAULT_FILTER_STATE, type FilterState } from './types';

const CONTENT_WIDTH = 'w-[1600px] max-w-[calc(100%-2rem)]';

export default function App() {
  const [filters, setFilters] = useState<FilterState>(DEFAULT_FILTER_STATE);
  const query = useCardsQuery(filters);

  const clearFilters = () => {
    setFilters(DEFAULT_FILTER_STATE);
  };

  return (
    <div className="min-h-screen bg-slate-950">
      <header className="border-b border-slate-800 bg-slate-900/80 px-4 py-4">
        <div className={`mx-auto ${CONTENT_WIDTH}`}>
          <h1 className="text-xl font-bold text-slate-50">Cards API Demo</h1>
          <p className="mt-1 text-sm text-slate-400">
            Live query builder for{' '}
            <code className="text-sky-300">GET /api/v2/cards</code>
          </p>
        </div>
      </header>

      <main
        className={`mx-auto grid ${CONTENT_WIDTH} gap-6 p-4 lg:grid-cols-[minmax(320px,540px)_minmax(0,1fr)] lg:gap-8 lg:py-6`}
      >
        <FilterPanel
          filters={filters}
          onChange={setFilters}
          onClear={clearFilters}
          handCostError={query.handCostError}
          reserveCostError={query.reserveCostError}
        />

        <div className="min-w-0 space-y-4">
          <QueryPreview queryString={query.queryString} url={query.url} />
          <ResultsPanel query={query} />
        </div>
      </main>
    </div>
  );
}
