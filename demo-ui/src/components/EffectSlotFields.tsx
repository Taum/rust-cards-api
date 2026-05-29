import type { EffectSlot } from '../types';

type EffectSlotFieldsProps = {
  slotIndex?: number;
  title?: string;
  slot: EffectSlot;
  onChange: (slot: EffectSlot) => void;
  onRemove?: () => void;
  removable?: boolean;
};

export function EffectSlotFields({
  slotIndex,
  title,
  slot,
  onChange,
  onRemove,
  removable = false,
}: EffectSlotFieldsProps) {
  const heading =
    title ?? (slotIndex !== undefined ? `Effect [${slotIndex}]` : 'Effect');
  const update = (field: keyof EffectSlot, value: string) => {
    onChange({ ...slot, [field]: value });
  };

  return (
    <div className="rounded-lg border border-slate-700 bg-slate-900/60 p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="text-sm font-medium text-slate-200">{heading}</h3>
        {removable && onRemove && (
          <button
            type="button"
            onClick={onRemove}
            className="rounded px-2 py-1 text-xs text-slate-400 hover:bg-slate-800 hover:text-red-300"
          >
            Remove
          </button>
        )}
      </div>
      <div className="grid gap-2 sm:grid-cols-3">
        <label className="block text-xs text-slate-400">
          Trigger (t)
          <input
            type="text"
            value={slot.t}
            onChange={(e) => update('t', e.target.value)}
            placeholder="e.g. 24,191"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
        <label className="block text-xs text-slate-400">
          Condition (c)
          <input
            type="text"
            value={slot.c}
            onChange={(e) => update('c', e.target.value)}
            placeholder="e.g. 191"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
        <label className="block text-xs text-slate-400">
          Output (o)
          <input
            type="text"
            value={slot.o}
            onChange={(e) => update('o', e.target.value)}
            placeholder="e.g. 90"
            className="mt-1 w-full rounded border border-slate-600 bg-slate-950 px-2 py-1.5 text-sm text-slate-100 placeholder:text-slate-600 focus:border-sky-500 focus:outline-none"
          />
        </label>
      </div>
    </div>
  );
}
