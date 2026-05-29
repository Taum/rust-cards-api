import type { CardV2 } from '../types';

type CardListPlaceholderProps = {
  cards: CardV2[];
};

function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) {
    return text;
  }
  return `${text.slice(0, maxLen)}…`;
}

export function CardListPlaceholder({ cards }: CardListPlaceholderProps) {
  if (cards.length === 0) {
    return (
      <p className="text-sm text-slate-500">No cards in this page.</p>
    );
  }

  return (
    <ul className="space-y-3">
      {cards.map((card) => {
        const mainText =
          card.mainEffect?.en_US ??
          card.mainEffect?.en_us ??
          Object.values(card.mainEffect ?? {})[0] ??
          '';

        return (
          <li
            key={card.reference}
            className="rounded-lg border border-slate-700 bg-slate-900/40 p-3"
          >
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <span className="font-mono text-sm text-sky-300">
                {card.reference}
              </span>
              <span className="text-xs text-slate-400">
                {card.faction.code} · hand {card.mainCost} · reserve{' '}
                {card.recallCost} · O/M/F {card.oceanPower}/
                {card.mountainPower}/{card.forestPower}
              </span>
            </div>
            <p className="mt-2 text-sm text-slate-300">
              {truncate(mainText, 200)}
            </p>
            <div
              data-card-ref={card.reference}
              className="mt-2 min-h-[2rem] rounded border border-dashed border-slate-700/80 bg-slate-950/50"
              aria-hidden
            />
          </li>
        );
      })}
    </ul>
  );
}
