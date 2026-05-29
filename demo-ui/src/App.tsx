import { useState } from 'react';
import { getApiBaseUrl } from './api/buildQuery';
import { FilterPanel } from './components/FilterPanel';
import { QueryPreview } from './components/QueryPreview';
import { ResultsPanel } from './components/ResultsPanel';
import { useCardsQuery } from './hooks/useCardsQuery';
import { useEffectsCatalog } from './hooks/useEffectsCatalog';
import { CARD_LOCALES, DEFAULT_CARD_LOCALE, type CardLocale } from './locale';
import { DEFAULT_FILTER_STATE, type FilterState } from './types';

const CONTENT_WIDTH = 'w-[1600px] max-w-[calc(100%-2rem)]';
const REPO_URL = 'https://github.com/Taum/rust-cards-api';

export default function App() {
  const [filters, setFilters] = useState<FilterState>(DEFAULT_FILTER_STATE);
  const [locale, setLocale] = useState<CardLocale>(DEFAULT_CARD_LOCALE);
  const query = useCardsQuery(filters);
  const effects = useEffectsCatalog();
  const effectsApiUrl = `${getApiBaseUrl()}/api/v2/effects`;

  const clearFilters = () => {
    setFilters(DEFAULT_FILTER_STATE);
  };

  return (
    <div className="flex h-full flex-col overflow-hidden bg-slate-950">
      <header className="shrink-0 border-b border-slate-800 bg-slate-900/80 px-4 py-4">
        <div
          className={`mx-auto flex flex-wrap items-center justify-between gap-4 ${CONTENT_WIDTH}`}
        >
          <div className="flex flex-wrap items-baseline gap-3">
            <h1 className="text-xl font-bold text-slate-50">Cards API Demo</h1>
            <a
              href={REPO_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-slate-400 hover:text-sky-300 hover:underline"
            >
              GitHub: Taum/rust-cards-api
            </a>
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <a
              href={effectsApiUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm inline-block flex flex-col group text-slate-50 hover:text-slate-600 hover:underline"
            >
              <h2>Show effects Catalog</h2>
              <code className="text-inherit text-sm text-sky-300 group-hover:text-slate-600">GET /api/v2/effects</code>
            </a>
            <label className="flex items-center gap-2 text-sm text-slate-300">
            <span className="text-slate-400">Locale</span>
            <select
              value={locale}
              onChange={(e) => setLocale(e.target.value as CardLocale)}
              className="rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 focus:border-sky-500 focus:outline-none"
            >
              {CARD_LOCALES.map(({ api, label }) => (
                <option key={api} value={api}>
                  {label}
                </option>
              ))}
            </select>
            </label>
          </div>
        </div>
      </header>

      <main
        className={`mx-auto grid min-h-0 flex-1 ${CONTENT_WIDTH} gap-6 p-4 lg:grid-cols-[minmax(320px,540px)_minmax(0,1fr)] lg:gap-8 lg:py-6`}
      >
        <FilterPanel
          filters={filters}
          onChange={setFilters}
          onClear={clearFilters}
          handCostError={query.handCostError}
          reserveCostError={query.reserveCostError}
          effectsCatalog={effects.catalog}
          effectsStatus={effects.status}
          effectsError={effects.error}
          locale={locale}
        />

        <div className="flex min-h-0 min-w-0 flex-col gap-4">
          <QueryPreview queryString={query.queryString} url={query.url} />
          <ResultsPanel
            query={query}
            locale={locale}
            showDebugTrigram={filters.debugBgaTrigram}
          />
        </div>
      </main>
    </div>
  );
}
