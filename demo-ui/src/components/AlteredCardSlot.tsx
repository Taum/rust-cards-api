import { useEffect, useRef } from 'react';

import { ensureAlteredRenderInit } from '../api/alteredRender';
import { cardToAlteredApiJson } from '../api/cardToAlteredApiJson';
import type { CardLocale } from '../locale';
import type { CardV2 } from '../types';

type AlteredCardSlotProps = {
  card: CardV2;
  locale: CardLocale;
};

export function AlteredCardSlot({ card, locale }: AlteredCardSlotProps) {
  const slotRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) {
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        await ensureAlteredRenderInit();
        if (cancelled) {
          return;
        }
        const apiJson = cardToAlteredApiJson(card, locale);
        await window.AlteredRender.mountFromApi(
          slot,
          apiJson as unknown as Record<string, unknown>,
        );
      } catch (err) {
        if (cancelled) {
          return;
        }
        const message = err instanceof Error ? err.message : String(err);
        slot.textContent = `Could not render card: ${message}`;
        console.error(err);
      }
    })();

    return () => {
      cancelled = true;
      slot.innerHTML = '';
    };
  }, [card, locale]);

  return (
    <div
      ref={slotRef}
      className="aspect-[744/1039] w-full overflow-hidden rounded border border-slate-700/80 bg-slate-950/50"
      data-card-ref={card.reference}
    />
  );
}
