import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES } from '@/i18n/languages';
import de from '@/i18n/locales/de.json';
import el from '@/i18n/locales/el.json';
import en from '@/i18n/locales/en.json';
import es from '@/i18n/locales/es.json';
import fr from '@/i18n/locales/fr.json';
import hi from '@/i18n/locales/hi.json';
import id from '@/i18n/locales/id.json';
import ja from '@/i18n/locales/ja.json';
import nl from '@/i18n/locales/nl.json';
import pt from '@/i18n/locales/pt.json';
import ru from '@/i18n/locales/ru.json';
import sv from '@/i18n/locales/sv.json';
import zh from '@/i18n/locales/zh.json';
// Imported by path rather than through the '@/stores' barrel, which would pull in every other store.
import { useSettingsStore } from '@/stores/settings';

i18n.use(initReactI18next).init({
    // Catalogs are bundled statically. This is a desktop app, so there is no HTTP backend and no lazy namespace
    // loading — which is also why <Suspense> is unnecessary and `useSuspense` is off below.
    resources: {
        de: { translation: de },
        el: { translation: el },
        en: { translation: en },
        es: { translation: es },
        fr: { translation: fr },
        hi: { translation: hi },
        id: { translation: id },
        ja: { translation: ja },
        nl: { translation: nl },
        pt: { translation: pt },
        ru: { translation: ru },
        sv: { translation: sv },
        zh: { translation: zh },
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
