import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LANGUAGES } from "./index.ts";
import { LANGUAGE_TAGS } from "./languages.ts";

/**
 * i18next resolves a plural by appending a CLDR category to the key, and languages have different
 * sets of categories: Japanese has only `other`, English has `one`/`other`, French adds `many`,
 * Russian adds `few`. The catalogues therefore differ in raw key count by design, and a naive diff
 * would flag every one of those languages.
 *
 * Normalising the suffix away is what makes the comparison meaningful: what must match across all
 * 13 files is the set of translatable strings, not the set of literal JSON keys.
 */
const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;

type Entry = { key: string; value: unknown };

const flatten = (value: unknown, prefix = ""): Entry[] => {
    if (value === null || typeof value !== "object") return [{ key: prefix, value }];

    return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
        flatten(child, prefix ? `${prefix}.${key}` : key),
    );
};

const keysOf = (language: keyof typeof CATALOGUES) =>
    new Set(flatten(CATALOGUES[language]).map(({ key }) => key.replace(PLURAL_SUFFIX, "")));

const reference = keysOf("en");

describe("locale catalogues", () => {
    it("has something to compare against", () => {
        expect(reference.size).toBeGreaterThan(100);
    });

    /**
     * The thirteen tags are spelled twice, and this is what holds the two spellings together.
     *
     * `i18n/languages.ts` is a leaf - the settings store imports it and `index.ts` imports the store,
     * so it cannot import the catalogues without closing that cycle - which leaves its list and the
     * keys of `CATALOGUES` as two independent statements of the same fact. Without this, adding a
     * language to one alone is silent in both directions: a tag here with no catalogue is a picker
     * entry that renders every string in English, and a catalogue with no tag here is a language that
     * ships and cannot be chosen.
     */
    it("ships exactly the languages the leaf module names", () => {
        expect([...LANGUAGE_TAGS].sort()).toEqual([...SUPPORTED_LANGUAGES].sort());
    });

    it.each(SUPPORTED_LANGUAGES)("%s has exactly the keys en has", (language) => {
        const keys = keysOf(language);

        // Reported as two sorted lists of dotted paths rather than as a bare count comparison: a
        // failure here is fixed by editing named keys, so the message has to name them.
        const missing = [...reference].filter((key) => !keys.has(key)).sort();
        const extra = [...keys].filter((key) => !reference.has(key)).sort();

        expect({ missing, extra }).toEqual({ missing: [], extra: [] });
    });

    // A key whose value is an empty string passes the set comparison above but renders as nothing on
    // screen, which is worse than a visible fallback to English.
    it.each(SUPPORTED_LANGUAGES)("%s has no empty translations", (language) => {
        const empty = flatten(CATALOGUES[language])
            .filter(({ value }) => typeof value === "string" && value.trim() === "")
            .map(({ key }) => key);

        expect(empty).toEqual([]);
    });
});
