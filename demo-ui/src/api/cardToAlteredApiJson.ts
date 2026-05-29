import { localeText, toRendererLang } from '../locale';
import type { CardV2 } from '../types';

export interface AlteredApiCardJson {
  reference: string;
  forge: { lang: string };
  cardRarity: { reference: string };
  faction: { name: string };
  set: { reference: string; code: string };
  cardType: { name: string };
  cardSubTypes: { reference: string; name: string }[];
  name: string;
  artist: string;
  artists: { name: string }[];
  mainCost: number;
  recallCost: number;
  forestPower: number;
  mountainPower: number;
  oceanPower: number;
  mainEffect: string;
  echoEffect: string | null;
}

const RARITY_BY_TOKEN: Record<string, string> = {
  U: 'UNIQUE',
  R: 'RARE',
  C: 'COMMON',
  E: 'EXALTED',
};

function rarityFromReference(reference: string): string {
  const rarityToken = reference.split('_').find((p) => p in RARITY_BY_TOKEN);
  return rarityToken
    ? (RARITY_BY_TOKEN[rarityToken] ?? 'UNIQUE')
    : 'UNIQUE';
}

export function cardToAlteredApiJson(
  card: CardV2,
  apiLocale: string,
): AlteredApiCardJson {
  const echo = localeText(card.echoEffect ?? {}, apiLocale);

  return {
    reference: card.reference,
    forge: { lang: toRendererLang(apiLocale) },
    cardRarity: { reference: rarityFromReference(card.reference) },
    faction: { name: card.faction.name },
    set: {
      reference: card.set.reference,
      code: card.set.code ?? '',
    },
    cardType: { name: 'Character' },
    cardSubTypes: card.cardSubTypes.map((subType) => ({
      reference: subType.reference,
      name: localeText(subType.name, apiLocale),
    })),
    name: localeText(card.name, apiLocale),
    artist: card.artist,
    artists: [{ name: card.artist }],
    mainCost: card.mainCost,
    recallCost: card.recallCost,
    forestPower: card.forestPower,
    mountainPower: card.mountainPower,
    oceanPower: card.oceanPower,
    mainEffect: localeText(card.mainEffect ?? {}, apiLocale),
    echoEffect: echo.length > 0 ? echo : null,
  };
}
