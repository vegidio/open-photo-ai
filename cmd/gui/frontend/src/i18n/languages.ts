// Leaf module: it must import nothing from the app, because `stores/settings.ts` imports `detectLanguage` from here
// and `i18n/index.ts` imports the store. Anything added here that reaches back into the app closes that cycle.

// Catalog ids are base language tags, deliberately without a region, and kept sorted by id. That order is for reading
// this file, not for the UI: the settings picker sorts by endonym, since the id is never shown to anyone. Every
// regional variation of a language shares one catalog, so `pt` serves pt-BR/pt-PT, `es` serves es-ES/es-MX/es-419,
// `de` serves de-DE/de-AT/de-CH, `nl` serves nl-NL/nl-BE, `fr` serves fr-FR/fr-CA and `zh` serves zh-CN/zh-SG. Where a
// language's regional dialects diverge in wording, the catalog picks one and states it: `es` is written in peninsular
// Spanish and `zh` in Simplified Chinese, which is why zh-TW/zh-HK also land on Simplified rather than Traditional.
// Splitting a language by region or script later means adding the regional id here and teaching detectLanguage() to
// prefer an exact match before falling back to the base tag.
export const SUPPORTED_LANGUAGES = [
    'de',
    'el',
    'en',
    'es',
    'fr',
    'hi',
    'id',
    'ja',
    'nl',
    'pt',
    'ru',
    'sv',
    'zh',
] as const;

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const DEFAULT_LANGUAGE: SupportedLanguage = 'en';

// Endonyms, deliberately NOT in the catalogs: someone who can't read the language the app is currently in must still
// recognise their own, so these have to render identically whatever language is active.
export const LANGUAGE_NAMES: Record<SupportedLanguage, string> = {
    de: 'Deutsch',
    el: 'Ελληνικά',
    en: 'English',
    es: 'Español',
    fr: 'Français',
    hi: 'हिन्दी',
    id: 'Bahasa Indonesia',
    ja: '日本語',
    nl: 'Nederlands',
    pt: 'Português',
    ru: 'Русский',
    sv: 'Svenska',
    zh: '简体中文',
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
        // `pt`, de-AT/de-CH to `de`, fr-CA/fr-BE to `fr`, nl-BE to `nl` and zh-CN/zh-Hans-CN to `zh`. Split on `_`
        // too: BCP-47 uses `-`, but a webview taking its language from a POSIX locale can hand us the `es_ES` form,
        // which `-` alone would leave unmatched.
        const base = tag?.toLowerCase().split(/[-_]/)[0];
        if (isSupportedLanguage(base)) return base;
    }

    return DEFAULT_LANGUAGE;
};
