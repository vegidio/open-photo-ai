import { afterEach, describe, expect, it, vi } from 'vitest';
import {
    DEFAULT_LANGUAGE,
    detectLanguage,
    isSupportedLanguage,
    LANGUAGE_NAMES,
    SUPPORTED_LANGUAGES,
} from './languages.ts';

const withLanguages = (languages: string[]) => vi.stubGlobal('navigator', { languages, language: languages[0] ?? '' });

afterEach(() => {
    vi.unstubAllGlobals();
});

describe('the supported language list', () => {
    it('names every language it claims to support', () => {
        for (const lang of SUPPORTED_LANGUAGES) {
            expect(LANGUAGE_NAMES[lang]).toBeTruthy();
        }
    });

    it('includes the default', () => {
        expect(SUPPORTED_LANGUAGES).toContain(DEFAULT_LANGUAGE);
    });
});

describe('isSupportedLanguage', () => {
    it('accepts a shipped language and rejects anything else', () => {
        expect(isSupportedLanguage('en')).toBe(true);
        expect(isSupportedLanguage('pt')).toBe(true);
        expect(isSupportedLanguage('en-US')).toBe(false);
        expect(isSupportedLanguage('kl')).toBe(false);
        expect(isSupportedLanguage(undefined)).toBe(false);
        expect(isSupportedLanguage(42)).toBe(false);
    });
});

describe('detectLanguage', () => {
    it('matches on the base tag', () => {
        withLanguages(['pt-BR']);
        expect(detectLanguage()).toBe('pt');
    });

    // A webview taking its language from a POSIX locale hands over `es_ES`, which splitting on `-` alone would miss.
    it('accepts the underscore form a POSIX locale produces', () => {
        withLanguages(['es_ES']);
        expect(detectLanguage()).toBe('es');
    });

    // The whole preference list is walked, so an unshipped primary locale falls through to the next choice rather
    // than dropping straight to English.
    it('walks past a language that is not shipped', () => {
        withLanguages(['kl-GL', 'fr-CA']);
        expect(detectLanguage()).toBe('fr');
    });

    it('falls back to the default when nothing matches', () => {
        withLanguages(['kl-GL', 'mi-NZ']);
        expect(detectLanguage()).toBe(DEFAULT_LANGUAGE);
    });

    it('uses navigator.language when the list is empty', () => {
        vi.stubGlobal('navigator', { languages: [], language: 'de-AT' });
        expect(detectLanguage()).toBe('de');
    });
});
