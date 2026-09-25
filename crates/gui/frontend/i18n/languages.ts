/**
 * The languages the application ships, named in themselves, and which of them this machine asks for.
 *
 * **A leaf: it imports nothing.** `stores/settings.ts` imports `detectLanguage` from here and
 * `i18n/index.ts` imports that store to read the persisted language, so anything added here that
 * reaches back into the application closes that cycle. That is also why the tags below are a list of
 * their own rather than `Object.keys(CATALOGUES)` - importing the catalogues would make this not a
 * leaf. The two spellings are held together by an assertion in `parity.test.ts` rather than by care.
 */

/**
 * The tags, as base language tags with no region, sorted by tag.
 *
 * That order is for reading this file, not for the picker: the tag is never shown to anyone, so the
 * settings row sorts by endonym instead. Every regional variation of a language shares one
 * catalogue - `pt` serves pt-BR and pt-PT, `es` serves es-ES/es-MX/es-419, `de` serves de-DE/AT/CH,
 * `nl` serves nl-NL/nl-BE, `fr` serves fr-FR/fr-CA and `zh` serves zh-CN/zh-SG. Where a language's
 * dialects diverge in wording the catalogue picks one and says so: `es` is peninsular Spanish and
 * `zh` is Simplified, which is why zh-TW and zh-HK also land on Simplified. Splitting a language by
 * region or script later means adding the regional tag here and teaching {@link detectLanguage} to
 * prefer an exact match before falling back to the base tag.
 */
export const LANGUAGE_TAGS = ["de", "el", "en", "es", "fr", "hi", "id", "ja", "nl", "pt", "ru", "sv", "zh"] as const;

export type Language = (typeof LANGUAGE_TAGS)[number];

/** The language everything falls back to: the one catalogue every other is translated from. */
export const DEFAULT_LANGUAGE: Language = "en";

/**
 * The endonyms - each language's name for itself.
 *
 * **Deliberately not in the catalogues.** Someone who cannot read the language the application is
 * currently showing must still recognise their own, which a list translated into the active language
 * would prevent: a Japanese speaker looking at a Greek interface needs to find 日本語, not Ιαπωνικά.
 */
export const LANGUAGE_NAMES: Record<Language, string> = {
    de: "Deutsch",
    el: "Ελληνικά",
    en: "English",
    es: "Español",
    fr: "Français",
    hi: "हिन्दी",
    id: "Bahasa Indonesia",
    ja: "日本語",
    nl: "Nederlands",
    pt: "Português",
    ru: "Русский",
    sv: "Svenska",
    zh: "简体中文",
};

/** Whether a value - a persisted setting, an environment tag - names a language the application ships. */
export const isLanguage = (value: unknown): value is Language =>
    typeof value === "string" && (LANGUAGE_TAGS as readonly string[]).includes(value);

/**
 * The language this machine asks for, where the application ships it, and English otherwise.
 *
 * **First launch only.** `zustand/persist` overwrites the store's initial value on every later boot,
 * so there is no "have I detected already?" flag to keep - a user who has chosen never reaches this
 * again, and a user who has not is asking the same question of the same environment.
 */
export const detectLanguage = (): Language => {
    // The ordered preference list rather than `navigator.language`, which is only its first entry.
    // Walking the whole list matters: someone whose primary locale the application does not ship but
    // whose second choice is pt-PT gets Portuguese rather than dropping straight to English.
    const tags = navigator.languages?.length ? navigator.languages : [navigator.language];

    for (const tag of tags) {
        // Matched on the base tag, so en-US and en-GB resolve to `en`, es-MX to `es`, pt-BR to `pt`
        // and zh-Hans-CN to `zh`. Split on `_` as well as `-`: BCP-47 uses the hyphen, but a webview
        // taking its language from a POSIX locale can hand over the `es_ES` form, which a hyphen
        // alone would leave unmatched.
        const base = tag?.toLowerCase().split(/[-_]/)[0];
        if (isLanguage(base)) return base;
    }

    return DEFAULT_LANGUAGE;
};
