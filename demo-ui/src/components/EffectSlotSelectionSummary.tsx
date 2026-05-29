import { useMemo } from 'react';
import { selectedEffectsForSlot } from '../api/selectedEffects';
import type { CardLocale } from '../locale';
import type { EffectSlot, EffectsCatalogResponse } from '../types';

type EffectSlotSelectionSummaryProps = {
  slot: EffectSlot;
  catalog: EffectsCatalogResponse | null;
  locale: CardLocale;
};

export function EffectSlotSelectionSummary({
  slot,
  catalog,
  locale,
}: EffectSlotSelectionSummaryProps) {
  const groups = useMemo(
    () => selectedEffectsForSlot(slot, catalog, locale),
    [slot, catalog, locale],
  );

  if (groups.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 space-y-3 border-t border-slate-700/80 pt-3">
      {groups.map((group) => (
        <div key={group.kind}>
          <h4 className="mb-1.5 text-xs font-medium text-slate-400">
            {group.kindLabel}
          </h4>
          <ul className="space-y-1.5 text-sm">
            {group.items.map((item, index) => (
              <li
                key={`${group.kind}-${item.idGd}-${index}`}
                className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 leading-snug"
              >
                <span className="shrink-0 font-medium text-sky-300">
                  {item.idGd}
                </span>
                <span className="text-slate-600">—</span>
                <span
                  className={
                    item.unknown ? 'text-amber-400/90 italic' : 'text-slate-300'
                  }
                >
                  {item.unknown
                    ? '(unknown id or catalog not loaded)'
                    : item.text || '(no text)'}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}
