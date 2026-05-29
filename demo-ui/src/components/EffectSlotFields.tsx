import { useMemo } from 'react';
import {
  catalogOptionsForRegion,
  type EffectRegion,
} from '../api/effectCatalogOptions';
import type { CardLocale } from '../locale';
import type { EffectCatalogItem, EffectSlot, EffectsCatalogResponse } from '../types';
import { EffectIdCombobox } from './EffectIdCombobox';
import { EffectSlotSelectionSummary } from './EffectSlotSelectionSummary';

type EffectSlotFieldsProps = {
  slotIndex?: number;
  title?: string;
  slot: EffectSlot;
  onChange: (slot: EffectSlot) => void;
  onRemove?: () => void;
  removable?: boolean;
  catalog: EffectsCatalogResponse | null;
  effectsLoading: boolean;
  locale: CardLocale;
  region: EffectRegion;
};

const EMPTY_OPTIONS: EffectCatalogItem[] = [];

export function EffectSlotFields({
  slotIndex,
  title,
  slot,
  onChange,
  onRemove,
  removable = false,
  catalog,
  effectsLoading,
  locale,
  region,
}: EffectSlotFieldsProps) {
  const heading =
    title ?? (slotIndex !== undefined ? `Effect [${slotIndex}]` : 'Effect');
  const update = (field: keyof EffectSlot, value: string) => {
    onChange({ ...slot, [field]: value });
  };

  const { triggers, conditions, output: outputs } = useMemo(() => {
    if (!catalog) {
      return {
        triggers: EMPTY_OPTIONS,
        conditions: EMPTY_OPTIONS,
        output: EMPTY_OPTIONS,
      };
    }
    return catalogOptionsForRegion(catalog, region);
  }, [catalog, region]);
  const comboboxDisabled = effectsLoading;

  return (
    <div className="overflow-visible rounded-lg border border-slate-700 bg-slate-900/60 p-3">
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
      <div className="grid gap-2 overflow-visible sm:grid-cols-3">
        <EffectIdCombobox
          label="Trigger (t)"
          value={slot.t}
          onChange={(v) => update('t', v)}
          options={triggers}
          locale={locale}
          disabled={comboboxDisabled}
          placeholder="e.g. 24,191"
        />
        <EffectIdCombobox
          label="Condition (c)"
          value={slot.c}
          onChange={(v) => update('c', v)}
          options={conditions}
          locale={locale}
          disabled={comboboxDisabled}
          placeholder="e.g. 191"
        />
        <EffectIdCombobox
          label="Output (o)"
          value={slot.o}
          onChange={(v) => update('o', v)}
          options={outputs}
          locale={locale}
          disabled={comboboxDisabled}
          placeholder="e.g. 90"
          menuAlign="end"
        />
      </div>
      <EffectSlotSelectionSummary
        slot={slot}
        catalog={catalog}
        locale={locale}
      />
    </div>
  );
}
