import { localeText, type CardLocale } from '../locale';
import type { CardV2, FamilyMatchV2 } from '../types';

import { AlteredCardSlot } from './AlteredCardSlot';

type CardListProps = {
  cards: CardV2[];
  locale: CardLocale;
  showDebugTrigram: boolean;
  withFamilies: boolean;
  families: FamilyMatchV2[] | null;
};

function familyCountByReference(
  families: FamilyMatchV2[] | null,
): Map<string, number> {
  const map = new Map<string, number>();
  if (!families) {
    return map;
  }
  for (const family of families) {
    map.set(family.reference, family.count);
  }
  return map;
}

export function CardList({
  cards,
  locale,
  showDebugTrigram,
  withFamilies,
  families,
}: CardListProps) {
  const matchCountByRef = familyCountByReference(families);
  if (cards.length === 0) {
    return (
      <p className="text-sm text-slate-500">No cards match these filters.</p>
    );
  }

  return (
    <div className="grid grid-cols-4 gap-3">
      {cards.map((card) => {
        const matchCount = matchCountByRef.get(card.reference);
        const caption =
          withFamilies && matchCount !== undefined
            ? `${localeText(card.name, locale)} (${matchCount.toLocaleString()})`
            : card.reference;

        return (
          <AlteredCardSlot
            key={card.reference}
            card={card}
            locale={locale}
            showDebugTrigram={showDebugTrigram}
            caption={caption}
          />
        );
      })}
    </div>
  );
}
