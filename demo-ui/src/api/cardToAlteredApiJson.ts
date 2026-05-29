import { localeText, toRendererLang } from '../locale';
import type { CardV2 } from '../types';

export interface AlteredApiCardJson {
  reference: string;
  forge: { lang: string };
  cardRarity: { reference: string };
  faction: { name: string };
  set: { reference: string; code: string };
  cardType: { name: string };
  cardSubTypes: { name: string }[];
  name: string;
  mainCost: number;
  recallCost: number;
  forestPower: number;
  mountainPower: number;
  oceanPower: number;
  mainEffect: string;
  echoEffect: string | null;
}

const FACTION_NAMES: Record<string, string> = {
  AX: 'Axiom',
  BR: 'Bravos',
  LY: 'Lyra',
  MU: 'Muna',
  OR: 'Ordis',
  YZ: 'Yzmir',
};

const RARITY_BY_TOKEN: Record<string, string> = {
  U: 'UNIQUE',
  R: 'RARE',
  C: 'COMMON',
  E: 'EXALTED',
};

function parseReference(reference: string): {
  setReference: string;
  factionCode: string;
  rarityReference: string;
} {
  const parts = reference.split('_');
  if (parts[0] !== 'ALT' || parts.length < 5) {
    return { setReference: '', factionCode: '', rarityReference: 'UNIQUE' };
  }

  const setReference = parts[1] ?? '';
  let i = 2;
  if (parts[i] === 'B') {
    i += 1;
  }
  const factionCode = parts[i] ?? '';

  const rarityToken = parts.find((p) => p in RARITY_BY_TOKEN);
  const rarityReference = rarityToken
    ? (RARITY_BY_TOKEN[rarityToken] ?? 'UNIQUE')
    : 'UNIQUE';

  return { setReference, factionCode, rarityReference };
}

export function cardToAlteredApiJson(
  card: CardV2,
  apiLocale: string,
): AlteredApiCardJson {
  const parsed = parseReference(card.reference);
  const echo = localeText(card.echoEffect ?? {}, apiLocale);

  return {
    reference: card.reference,
    forge: { lang: toRendererLang(apiLocale) },
    cardRarity: { reference: parsed.rarityReference },
    faction: {
      name:
        FACTION_NAMES[card.faction.code] ??
        FACTION_NAMES[parsed.factionCode] ??
        card.faction.code,
    },
    set: {
      reference: card.set?.reference ?? parsed.setReference,
      code: card.set?.code ?? '',
    },
    cardType: { name: 'Character' },
    cardSubTypes: [],
    name: card.reference,
    mainCost: card.mainCost,
    recallCost: card.recallCost,
    forestPower: card.forestPower,
    mountainPower: card.mountainPower,
    oceanPower: card.oceanPower,
    mainEffect: localeText(card.mainEffect ?? {}, apiLocale),
    echoEffect: echo.length > 0 ? echo : null,
  };
}
