import { useEffect, useRef, useState } from 'react';

import { ensureAlteredRenderInit } from '../api/alteredRender';
import { withAlteredMountSlot } from '../api/alteredMountQueue';
import { cardToAlteredApiJson } from '../api/cardToAlteredApiJson';
import { useResultsScrollRoot } from '../context/ResultsScrollContext';
import type { CardLocale } from '../locale';
import type { CardV2 } from '../types';

const CARD_SHELL_CLASS =
  'relative aspect-[744/1039] w-full overflow-hidden rounded-lg bg-slate-900';
const LAZY_ROOT_MARGIN = '200px';

function clearMountNode(node: HTMLElement): void {
  node.replaceChildren();
}

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
  const scrollRoot = useResultsScrollRoot();
  const observeRef = useRef<HTMLDivElement>(null);
  /** Imperative-only mount target — never put React children here. */
  const mountRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [mounted, setMounted] = useState(false);
  const [mountError, setMountError] = useState<string | null>(null);
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
    const target = observeRef.current;
    if (!target) {
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        setVisible(entry.isIntersecting);
      },
      {
        root: scrollRoot,
        rootMargin: LAZY_ROOT_MARGIN,
        threshold: 0,
      },
    );

    observer.observe(target);
    return () => observer.disconnect();
  }, [scrollRoot]);

  useEffect(() => {
    if (!visible) {
      setMounted(false);
      setMountError(null);
      const mountEl = mountRef.current;
      if (mountEl) {
        clearMountNode(mountEl);
      }
      return;
    }

    const mountEl = mountRef.current;
    if (!mountEl) {
      return;
    }

    let cancelled = false;

    void (async () => {
      setMountError(null);
      setMounted(false);
      clearMountNode(mountEl);

      try {
        await withAlteredMountSlot(async () => {
          if (cancelled) {
            return;
          }
          await ensureAlteredRenderInit();
          if (cancelled) {
            return;
          }
          const apiJson = cardToAlteredApiJson(card, locale);
          await window.AlteredRender.mountFromApi(
            mountEl,
            apiJson as unknown as Record<string, unknown>,
          );
        });

        if (!cancelled) {
          setMounted(true);
        }
      } catch (err) {
        if (cancelled) {
          return;
        }
        clearMountNode(mountEl);
        const message = err instanceof Error ? err.message : String(err);
        setMountError(message);
        console.error(err);
      }
    })();

    return () => {
      cancelled = true;
      clearMountNode(mountEl);
    };
  }, [visible, card, locale]);

  const showPlaceholder = visible && !mounted && mountError === null;

  return (
    <div ref={observeRef} className="flex flex-col gap-1.5">
      <div className={CARD_SHELL_CLASS} data-card-ref={card.reference}>
        <div ref={mountRef} className="absolute inset-0 h-full w-full" />
        {showPlaceholder && (
          <div
            className="pointer-events-none absolute inset-0 z-10 rounded-lg border border-slate-700/80 bg-slate-800/90"
            aria-hidden
          />
        )}
        {mountError !== null && (
          <p className="absolute inset-0 z-20 flex items-center justify-center p-2 text-center text-xs text-red-400">
            Could not render card: {mountError}
          </p>
        )}
      </div>
      <p
        className="truncate px-0.5 text-center text-xs leading-tight tracking-tight text-slate-500 [font-family:'Roboto_Condensed','Arial_Narrow','Liberation_Sans_Narrow',ui-sans-serif,sans-serif]"
        title={
          caption === card.reference ? card.reference : `${caption} — ${card.reference}`
        }
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
