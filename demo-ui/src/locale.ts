export const CARD_LOCALES = [
  { api: 'en_US', label: 'English', renderer: 'en' },
  { api: 'fr_FR', label: 'Français', renderer: 'fr' },
  { api: 'de_DE', label: 'Deutsch', renderer: 'de' },
  { api: 'es_ES', label: 'Español', renderer: 'es' },
  { api: 'it_IT', label: 'Italiano', renderer: 'it' },
] as const;

export type CardLocale = (typeof CARD_LOCALES)[number]['api'];

export const DEFAULT_CARD_LOCALE: CardLocale = 'en_US';

export function localeText(
  map: Record<string, string>,
  apiLocale: string,
): string {
  return (
    map[apiLocale] ??
    map.en_US ??
    map.en_us ??
    Object.values(map)[0] ??
    ''
  );
}

export function toRendererLang(apiLocale: string): string {
  return CARD_LOCALES.find((l) => l.api === apiLocale)?.renderer ?? 'en';
}
