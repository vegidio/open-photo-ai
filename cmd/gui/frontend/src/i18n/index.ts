import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES } from '@/i18n/languages';
import en from '@/i18n/locales/en.json';
import pt from '@/i18n/locales/pt.json';
// Imported by path rather than through the '@/stores' barrel, which would pull in every other store.
import { useSettingsStore } from '@/stores/settings';

i18n.use(initReactI18next).init({
    // Catalogs are bundled statically. This is a desktop app, so there is no HTTP backend and no lazy namespace
    // loading — which is also why <Suspense> is unnecessary and `useSuspense` is off below.
    resources: {
        en: { translation: en },
        pt: { translation: pt },
    },
    // The store is the source of truth and rehydrates from localStorage synchronously at import time, so the correct
    // language is known before the first render — no flash of English on a Portuguese install.
    lng: useSettingsStore.getState().language,
    fallbackLng: DEFAULT_LANGUAGE,
    supportedLngs: SUPPORTED_LANGUAGES,
    // React escapes everything it renders; leaving this on would double-escape apostrophes.
    interpolation: { escapeValue: false },
    returnNull: false,
    react: { useSuspense: false },
});

// Keep <html lang> tracking the UI language: it drives hyphenation, spellcheck and :lang() inside the webview.
const syncDocumentLang = (lng: string) => {
    document.documentElement.lang = lng;
};

syncDocumentLang(i18n.resolvedLanguage ?? DEFAULT_LANGUAGE);
i18n.on('languageChanged', syncDocumentLang);

export default i18n;
