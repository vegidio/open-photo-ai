// Leaf module: it must import nothing from the app, because `stores/settings.ts` imports `detectLanguage` from here
// and `i18n/index.ts` imports the store. Anything added here that reaches back into the app closes that cycle.

// Catalog ids are base language tags, deliberately without a region. Every regional variation of a language shares
// one catalog, so `pt` serves pt-BR/pt-PT, `es` serves es-ES/es-MX/es-419, `sv` serves sv-SE/sv-FI and `ru` serves
// ru-RU/ru-BY/ru-KZ. Where a language's regional dialects diverge in wording, the catalog picks one and states it:
// `es` is written in peninsular Spanish. Splitting a language by region later means adding the regional id here and
// teaching detectLanguage() to prefer an exact match before falling back to the base tag.
export const SUPPORTED_LANGUAGES = ['en', 'es', 'pt', 'ru', 'sv'] as const;

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const DEFAULT_LANGUAGE: SupportedLanguage = 'en';

// Endonyms, deliberately NOT in the catalogs: someone who can't read the language the app is currently in must still
// recognise their own, so these have to render identically whatever language is active.
export const LANGUAGE_NAMES: Record<SupportedLanguage, string> = {
    en: 'English',
    es: 'Español',
    pt: 'Português',
    ru: 'Русский',
    sv: 'Svenska',
};

export const isSupportedLanguage = (value: unknown): value is SupportedLanguage =>
    typeof value === 'string' && (SUPPORTED_LANGUAGES as readonly string[]).includes(value);

// First launch only: zustand's `persist` overwrites this initial value on every subsequent boot, so there is no
// "have I detected already?" flag to keep.
export const detectLanguage = (): SupportedLanguage => {
    // `navigator.languages` is the ordered preference list; `navigator.language` is only its first entry. Walking the
    // whole list matters — someone whose primary locale we don't ship but whose second choice is pt-PT gets
    // Portuguese rather than dropping straight to English.
    const tags = navigator.languages?.length ? navigator.languages : [navigator.language];

    for (const tag of tags) {
        // Compared on the base tag, so en-US/en-GB all resolve to `en`, es-ES/es-MX/es-419 to `es`, pt-BR/pt-PT to
        // `pt`, ru-RU/ru-BY to `ru` and sv-SE/sv-FI to `sv`. Split on `_` too: BCP-47 uses `-`, but a webview taking
        // its language from a POSIX locale can hand us the `es_ES` form, which `-` alone would leave unmatched.
        const base = tag?.toLowerCase().split(/[-_]/)[0];
        if (isSupportedLanguage(base)) return base;
    }

    return DEFAULT_LANGUAGE;
};
