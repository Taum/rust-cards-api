import type { EffectCatalogItem, EffectsCatalogResponse } from '../types';

export type EffectRegion = 'main' | 'echo';

function matchesRegion(item: EffectCatalogItem, region: EffectRegion): boolean {
  return region === 'main' ? item.isMain === true : item.isEcho === true;
}

/** Catalog options for main effect slots or support (echo), filtered by isMain / isEcho. */
export function catalogOptionsForRegion(
  catalog: EffectsCatalogResponse,
  region: EffectRegion,
): {
  triggers: EffectCatalogItem[];
  conditions: EffectCatalogItem[];
  output: EffectCatalogItem[];
} {
  const filter = (items: EffectCatalogItem[]) =>
    items.filter((item) => matchesRegion(item, region));

  return {
    triggers: filter(catalog.triggers),
    conditions: filter(catalog.conditions),
    output: filter(catalog.output),
  };
}
