import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { DEFAULT_LANGUAGE } from "@/i18n/languages";
import de from "@/i18n/locales/de.json";
import el from "@/i18n/locales/el.json";
import en from "@/i18n/locales/en.json";
import es from "@/i18n/locales/es.json";
import fr from "@/i18n/locales/fr.json";
import hi from "@/i18n/locales/hi.json";
import id from "@/i18n/locales/id.json";
import ja from "@/i18n/locales/ja.json";
import nl from "@/i18n/locales/nl.json";
import pt from "@/i18n/locales/pt.json";
import ru from "@/i18n/locales/ru.json";
import sv from "@/i18n/locales/sv.json";
import zh from "@/i18n/locales/zh.json";
import { useSettingsStore } from "@/stores/settings";

// Re-exported from the leaf that owns it rather than declared a second time here. `languages.ts`
// must import nothing - the settings store imports it and this file imports that store - so the
// fallback is stated there, and this keeps the name every existing caller already imports.
export { DEFAULT_LANGUAGE };

/**
 * The catalogues, keyed by language tag.
 *
 * Exported because the parity test asserts across exactly this map. Reading the directory instead
 * would need `node:fs` in a browser tsconfig, and would check what happens to be on disk rather than
 * what the application actually bundles - a catalogue that existed but was never imported here would
 * pass that version of the test and still be missing at runtime.
 */
export const CATALOGUES = { de, el, en, es, fr, hi, id, ja, nl, pt, ru, sv, zh } as const;

/** The language tags the application ships a catalogue for. */
export const SUPPORTED_LANGUAGES = Object.keys(CATALOGUES) as (keyof typeof CATALOGUES)[];

i18n.use(initReactI18next).init({
    // Bundled statically. This is a desktop application: there is no HTTP backend to lazily load a
    // namespace from, which is also why <Suspense> is unnecessary and `useSuspense` is off below.
    resources: Object.fromEntries(
        Object.entries(CATALOGUES).map(([language, translation]) => [language, { translation }]),
    ),
    /*
     * The user's own choice, or - on a first launch - what their environment asked for, which is
     * what the store's initial value is.
     *
     * **Read here rather than applied by an effect**, which is what makes initialization order
     * load-bearing: it rests on `zustand/persist` over `localStorage` rehydrating synchronously, so
     * the store's language is already correct at the moment this module runs. The alternative - a
     * `changeLanguage` in a mount effect - costs one frame of English on every launch for every user
     * who is not English, and is the fallback if that ever stops being true, since the symptom would
     * be that frame rather than a crash.
     */
    lng: useSettingsStore.getState().language,
    fallbackLng: DEFAULT_LANGUAGE,
    supportedLngs: SUPPORTED_LANGUAGES,
    // React escapes everything it renders; leaving this on would double-escape apostrophes.
    interpolation: { escapeValue: false },
    returnNull: false,
    react: { useSuspense: false },
});

export default i18n;
