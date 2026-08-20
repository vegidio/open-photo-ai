import type en from '@/i18n/locales/en.json';

// Types every t() call against the English catalog, so a typo'd or removed key fails `pnpm typecheck` rather than
// silently rendering the key itself at runtime. English is the reference catalog; other languages may lag behind it
// and fall back per-key via `fallbackLng`.
declare module 'i18next' {
    interface CustomTypeOptions {
        defaultNS: 'translation';
        returnNull: false;
        resources: {
            translation: typeof en;
        };
    }
}
