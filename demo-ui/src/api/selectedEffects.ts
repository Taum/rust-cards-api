import { localeText, type CardLocale } from '../locale';
import type {
  EffectCatalogItem,
  EffectsCatalogResponse,
  EffectSlot,
} from '../types';

export type EffectFieldKind = 't' | 'c' | 'o';

export type SelectedEffectLine = {
  idGd: number;
  text: string;
  unknown: boolean;
};

export type SelectedEffectGroup = {
  kind: EffectFieldKind;
  kindLabel: string;
  items: SelectedEffectLine[];
};

const KIND_LABEL: Record<EffectFieldKind, string> = {
  t: 'Trigger',
  c: 'Condition',
  o: 'Output',
};

const FIELD_ORDER: EffectFieldKind[] = ['t', 'c', 'o'];

export function parseIdGdList(raw: string): number[] {
  return raw
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => Number.parseInt(part, 10))
    .filter((id) => !Number.isNaN(id));
}

function lookupMaps(catalog: EffectsCatalogResponse) {
  const toMap = (items: EffectCatalogItem[]) =>
    new Map(items.map((item) => [item.idGd, item]));

  return {
    t: toMap(catalog.triggers),
    c: toMap(catalog.conditions),
    o: toMap(catalog.output),
  };
}

/** Selected idGd lines for one slot, grouped by trigger / condition / output. */
export function selectedEffectsForSlot(
  slot: EffectSlot,
  catalog: EffectsCatalogResponse | null,
  locale: CardLocale,
): SelectedEffectGroup[] {
  if (!catalog) {
    return [];
  }

  const maps = lookupMaps(catalog);
  const groups: SelectedEffectGroup[] = [];

  for (const kind of FIELD_ORDER) {
    const items: SelectedEffectLine[] = [];
    for (const idGd of parseIdGdList(slot[kind])) {
      const item = maps[kind].get(idGd);
      items.push({
        idGd,
        text: item ? localeText(item.text, locale) : '',
        unknown: !item,
      });
    }
    if (items.length > 0) {
      groups.push({
        kind,
        kindLabel: KIND_LABEL[kind],
        items,
      });
    }
  }

  return groups;
}
