import { activeEffectSlotCount } from '../api/buildQuery';
import type { CardLocale } from '../locale';
import {
  FACTIONS,
  SOURCE_SETS,
  type EffectMode,
  type EffectsCatalogResponse,
  type EffectsCatalogStatus,
  type FilterState,
} from '../types';
import { EffectSlotFields } from './EffectSlotFields';

type FilterPanelProps = {
  filters: FilterState;
  onChange: (filters: FilterState) => void;
  onClear: () => void;
  handCostError?: string | null;
  reserveCostError?: string | null;
  effectsCatalog: EffectsCatalogResponse | null;
  effectsStatus: EffectsCatalogStatus;
  effectsError: string | null;
  locale: CardLocale;
};

export function FilterPanel({
  filters,
  onChange,
  onClear,
  handCostError,
  reserveCostError,
  effectsCatalog,
  effectsStatus,
  effectsError,
  locale,
}: FilterPanelProps) {
  const effectsLoading = effectsStatus === 'loading';
  const setFilters = (patch: Partial<FilterState>) => {
    onChange({ ...filters, ...patch });
  };

  const updateEffectSlot = (index: number, slot: FilterState['effects'][number]) => {
    const effects = [...filters.effects];
    effects[index] = slot;
    setFilters({ effects });
  };

  const addEffectSlot = () => {
    setFilters({
      effects: [...filters.effects, { t: '', c: '', o: '' }],
    });
  };

  const removeEffectSlot = (index: number) => {
    if (filters.effects.length <= 1) {
      return;
    }
    setFilters({
      effects: filters.effects.filter((_, i) => i !== index),
    });
  };

  const toggleFaction = (code: string) => {
    const next = filters.factions.includes(code)
      ? filters.factions.filter((f) => f !== code)
      : [...filters.factions, code];
    setFilters({ factions: next });
  };

  const toggleSet = (code: string) => {
    const next = filters.sets.includes(code)
      ? filters.sets.filter((s) => s !== code)
      : [...filters.sets, code];
    setFilters({ sets: next });
  };

  const multipleEffects = activeEffectSlotCount(filters) >= 2;

  const setEffectMode = (effectMode: EffectMode) => {
    setFilters({ effectMode });
  };

  return (
    <aside className="min-h-0 space-y-5 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-100">Filters</h2>
        <button
          type="button"
          onClick={onClear}
          className="rounded border border-slate-600 px-2 py-1 text-xs text-slate-300 hover:bg-slate-800"
        >
          Clear filters
        </button>
      </div>

      <section className="space-y-3 overflow-visible">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-sm font-medium text-slate-300">Effects</h3>
          {multipleEffects && (
            <div
              className="flex rounded border border-slate-600 text-sm"
              role="group"
              aria-label="Effect combine mode"
            >
              <button
                type="button"
                onClick={() => setEffectMode('and')}
                className={`px-3 py-1 font-medium ${
                  filters.effectMode === 'and'
                    ? 'bg-sky-600 text-white'
                    : 'text-slate-300 hover:bg-slate-800'
                }`}
                aria-pressed={filters.effectMode === 'and'}
              >
                AND
              </button>
              <button
                type="button"
                onClick={() => setEffectMode('or')}
                className={`border-l border-slate-600 px-3 py-1 font-medium ${
                  filters.effectMode === 'or'
                    ? 'bg-sky-600 text-white'
                    : 'text-slate-300 hover:bg-slate-800'
                }`}
                aria-pressed={filters.effectMode === 'or'}
              >
                OR
              </button>
            </div>
          )}
        </div>
        {effectsLoading && (
          <p className="text-xs text-slate-500">Loading effects catalog…</p>
        )}
        {effectsStatus === 'error' && effectsError && (
          <p className="text-xs text-amber-400">
            Could not load effects list ({effectsError}). You can still type idGd
            values manually.
          </p>
        )}
        {filters.effects.map((slot, index) => (
          <EffectSlotFields
            key={index}
            slotIndex={index}
            slot={slot}
            onChange={(next) => updateEffectSlot(index, next)}
            removable={filters.effects.length > 1}
            onRemove={() => removeEffectSlot(index)}
            catalog={effectsCatalog}
            effectsLoading={effectsLoading}
            locale={locale}
            region="main"
            filters={filters}
          />
        ))}
        <button
          type="button"
          onClick={addEffectSlot}
          className="w-full rounded border border-dashed border-slate-600 py-2 text-sm text-slate-400 hover:border-sky-600 hover:text-sky-300"
        >
          Add effect slot
        </button>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-slate-300">Support</h3>
        <EffectSlotFields
          title="Support"
          slot={filters.support}
          onChange={(support) => setFilters({ support })}
          catalog={effectsCatalog}
          effectsLoading={effectsLoading}
          locale={locale}
          region="echo"
          filters={filters}
        />
      </section>

      <section className="space-y-2">
        <label className="block text-sm text-slate-300">
          Card reference
          <input
            type="text"
            value={filters.reference}
            onChange={(e) => setFilters({ reference: e.target.value })}
            placeholder="ALT_CYCLONE_B_BR_77_U_1787"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
      </section>

      <section className="space-y-2">
        <label className="block text-sm text-slate-300">
          Character name
          <input
            type="text"
            value={filters.name}
            onChange={(e) => setFilters({ name: e.target.value })}
            placeholder="Substring match, e.g. Kelon"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-slate-300">Factions</h3>
        <div className="flex flex-wrap gap-3">
          {FACTIONS.map((code) => (
            <label
              key={code}
              className="flex cursor-pointer items-center gap-1.5 text-sm text-slate-200"
            >
              <input
                type="checkbox"
                checked={filters.factions.includes(code)}
                onChange={() => toggleFaction(code)}
                className="rounded border-slate-600 bg-slate-950 text-sky-500 focus:ring-sky-500"
              />
              {code}
            </label>
          ))}
        </div>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-slate-300">Sets</h3>
        <div className="flex flex-wrap gap-3">
          {SOURCE_SETS.map((code) => (
            <label
              key={code}
              className="flex cursor-pointer items-center gap-1.5 text-sm text-slate-200"
            >
              <input
                type="checkbox"
                checked={filters.sets.includes(code)}
                onChange={() => toggleSet(code)}
                className="rounded border-slate-600 bg-slate-950 text-sky-500 focus:ring-sky-500"
              />
              {code}
            </label>
          ))}
        </div>
      </section>

      <section className="grid gap-3 sm:grid-cols-2">
        <label className="block text-sm text-slate-300">
          Hand cost
          <input
            type="text"
            value={filters.handCost}
            onChange={(e) => setFilters({ handCost: e.target.value })}
            placeholder="2-5 or 2,4,6 or 3"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
          {handCostError && (
            <span className="mt-1 block text-xs text-red-400">{handCostError}</span>
          )}
        </label>
        <label className="block text-sm text-slate-300">
          Reserve cost
          <input
            type="text"
            value={filters.reserveCost}
            onChange={(e) => setFilters({ reserveCost: e.target.value })}
            placeholder="2-5 or 2,4,6 or 3"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
          {reserveCostError && (
            <span className="mt-1 block text-xs text-red-400">
              {reserveCostError}
            </span>
          )}
        </label>
      </section>

      <section>
        <label className="block text-sm text-slate-300">
          Limit (1–200)
          <input
            type="number"
            min={1}
            max={200}
            value={filters.limit}
            onChange={(e) => setFilters({ limit: e.target.value })}
            placeholder="50"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
      </section>

      <section className="border-t border-slate-800 pt-4">
        <label className="flex cursor-pointer items-center gap-2 text-sm text-slate-200">
          <input
            type="checkbox"
            checked={filters.withFamilies}
            onChange={(e) => setFilters({ withFamilies: e.target.checked })}
            className="rounded border-slate-600 bg-slate-950 text-sky-500 focus:ring-sky-500"
          />
          Group by family
        </label>
        <p className="mt-1 text-xs text-slate-500">
          Adds <code className="font-mono text-slate-400">withFamilies</code> on
          the first request: one card per matching family (full prints use a
          follow-up without this flag).
        </p>
      </section>

      <section className="border-t border-slate-800 pt-4">
        <h3 className="mb-2 text-sm font-medium text-slate-300">Debug</h3>
        <label className="flex cursor-pointer items-center gap-2 text-sm text-slate-200">
          <input
            type="checkbox"
            checked={filters.debugBgaTrigram}
            onChange={(e) => setFilters({ debugBgaTrigram: e.target.checked })}
            className="rounded border-slate-600 bg-slate-950 text-sky-500 focus:ring-sky-500"
          />
          BGA trigrams
        </label>
        <p className="mt-1 text-xs text-slate-500">
          Adds <code className="font-mono text-slate-400">debug_bga_trigram</code> to
          the API request and shows TCO triplets under each card.
        </p>
      </section>
    </aside>
  );
}
