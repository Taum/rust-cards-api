import { EffectSlotFields } from './EffectSlotFields';
import { FACTIONS, type FilterState } from '../types';

type FilterPanelProps = {
  filters: FilterState;
  onChange: (filters: FilterState) => void;
  onClear: () => void;
  handCostError?: string | null;
  reserveCostError?: string | null;
};

export function FilterPanel({
  filters,
  onChange,
  onClear,
  handCostError,
  reserveCostError,
}: FilterPanelProps) {
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
    if (index === 0) {
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

  return (
    <aside className="space-y-5">
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

      <section className="space-y-3">
        <h3 className="text-sm font-medium text-slate-300">Effects</h3>
        {filters.effects.map((slot, index) => (
          <EffectSlotFields
            key={index}
            slotIndex={index}
            slot={slot}
            onChange={(next) => updateEffectSlot(index, next)}
            removable={index > 0}
            onRemove={() => removeEffectSlot(index)}
          />
        ))}
        <button
          type="button"
          onClick={addEffectSlot}
          className="w-full rounded border border-dashed border-slate-600 py-2 text-sm text-slate-400 hover:border-sky-600 hover:text-sky-300"
        >
          Add effect slot
        </button>
        {filters.effects.length > 1 && (
          <p className="text-xs text-amber-400/90">
            API multi-slot support pending — queries with effect[1]+ may return
            400 until the backend is updated.
          </p>
        )}
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-medium text-slate-300">Support</h3>
        <EffectSlotFields
          title="Support"
          slot={filters.support}
          onChange={(support) => setFilters({ support })}
        />
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

      <section className="grid gap-3 sm:grid-cols-2">
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
        <label className="block text-sm text-slate-300">
          Cursor
          <input
            type="text"
            value={filters.cursor}
            onChange={(e) => setFilters({ cursor: e.target.value })}
            placeholder="optional"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
      </section>
    </aside>
  );
}
