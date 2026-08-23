import { describe, expect, it } from 'vitest';
import { SUPPORTED_LANGUAGES } from './languages.ts';
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

// The catalogs are imported the same way i18n/index.ts imports them, rather than read off disk. That keeps this test
// inside the browser tsconfig (no node:fs, no @types/node leaking into app code) and, more usefully, checks what the
// app actually bundles instead of what happens to be sitting in the directory.
const CATALOGS: Record<string, unknown> = { de, el, en, es, fr, hi, id, ja, nl, pt, ru, sv, zh };

// i18next resolves a plural by appending a CLDR category to the key, and languages have different sets of categories:
// Japanese has only `other`, English has `one`/`other`, French adds `many`, Russian adds `few`. The catalogs therefore
// differ in raw key count by design, and a naive diff would flag every one of those languages.
//
// Normalising the suffix away is what makes the comparison meaningful: what must match across all 13 files is the set
// of translatable strings, not the set of literal JSON keys.
const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;

type Entry = { key: string; value: unknown };

const flatten = (value: unknown, prefix = ''): Entry[] => {
    if (value === null || typeof value !== 'object') return [{ key: prefix, value }];

    return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
        flatten(child, prefix ? `${prefix}.${key}` : key),
    );
};

const keysOf = (lang: string) => new Set(flatten(CATALOGS[lang]).map(({ key }) => key.replace(PLURAL_SUFFIX, '')));

const reference = keysOf('en');

describe('locale catalogs', () => {
    it('bundles a catalog for every supported language, and no others', () => {
        expect(Object.keys(CATALOGS).sort()).toEqual([...SUPPORTED_LANGUAGES].sort());
    });

    it('has something to compare against', () => {
        expect(reference.size).toBeGreaterThan(100);
    });

    it.each([...SUPPORTED_LANGUAGES])('%s has exactly the keys en has', (lang) => {
        const keys = keysOf(lang);

        const missing = [...reference].filter((key) => !keys.has(key)).sort();
        const extra = [...keys].filter((key) => !reference.has(key)).sort();

        expect({ missing, extra }).toEqual({ missing: [], extra: [] });
    });

    // A key whose value is an empty string passes the set comparison above but renders as nothing on screen, which is
    // worse than a visible fallback to English.
    it.each([...SUPPORTED_LANGUAGES])('%s has no empty translations', (lang) => {
        const empty = flatten(CATALOGS[lang])
            .filter(({ value }) => typeof value === 'string' && value.trim() === '')
            .map(({ key }) => key);

        expect(empty).toEqual([]);
    });
});
