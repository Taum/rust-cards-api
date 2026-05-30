import { useEffect, useRef, useState } from 'react';

import { ensureAlteredRenderInit } from '../api/alteredRender';
import { cardToAlteredApiJson } from '../api/cardToAlteredApiJson';
import type { CardLocale } from '../locale';
import type { CardV2 } from '../types';

type AlteredCardSlotProps = {
  card: CardV2;
  locale: CardLocale;
  showDebugTrigram: boolean;
  caption: string;
};

export function AlteredCardSlot({
  card,
  locale,
  showDebugTrigram,
  caption,
}: AlteredCardSlotProps) {
  const slotRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);
  const trigramText = card.debug_bga_trigram ?? '';

  const copyTrigram = async () => {
    if (!trigramText) {
      return;
    }
    try {
      await navigator.clipboard.writeText(trigramText);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };

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
    <div className="flex flex-col gap-1.5">
      <div
        ref={slotRef}
        className="aspect-[744/1039] w-full overflow-hidden rounded-lg"
        data-card-ref={card.reference}
      />
      <p
        className="truncate px-0.5 text-center text-xs leading-tight tracking-tight text-slate-500 [font-family:'Roboto_Condensed','Arial_Narrow','Liberation_Sans_Narrow',ui-sans-serif,sans-serif]"
        title={caption === card.reference ? card.reference : `${caption} — ${card.reference}`}
      >
        {caption}
      </p>
      {showDebugTrigram && (
        <div className="flex items-start gap-1">
          <p className="min-w-0 flex-1 break-all font-mono text-[10px] leading-snug text-slate-400">
            {trigramText}
          </p>
          <button
            type="button"
            onClick={() => void copyTrigram()}
            disabled={!trigramText}
            title={copied ? 'Copied' : 'Copy trigram'}
            aria-label={copied ? 'Copied' : 'Copy trigram'}
            className={`shrink-0 rounded border px-1 py-0.5 font-mono text-[10px] leading-none disabled:opacity-40 ${
              copied
                ? 'border-emerald-500 bg-emerald-950/60 text-emerald-400'
                : 'border-slate-600 text-slate-400 hover:bg-slate-800 hover:text-slate-200'
            }`}
          >
            C
          </button>
        </div>
      )}
    </div>
  );
}
