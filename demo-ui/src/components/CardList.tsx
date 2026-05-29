import type { CardLocale } from '../locale';
import type { CardV2 } from '../types';

import { AlteredCardSlot } from './AlteredCardSlot';

type CardListProps = {
  cards: CardV2[];
  locale: CardLocale;
  showDebugTrigram: boolean;
};

export function CardList({ cards, locale, showDebugTrigram }: CardListProps) {
  if (cards.length === 0) {
    return (
      <p className="text-sm text-slate-500">No cards match these filters.</p>
    );
  }

  return (
    <div className="grid grid-cols-4 gap-3">
      {cards.map((card) => (
        <AlteredCardSlot
          key={card.reference}
          card={card}
          locale={locale}
          showDebugTrigram={showDebugTrigram}
        />
      ))}
    </div>
  );
}
