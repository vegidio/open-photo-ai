import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_LANGUAGE, detectLanguage, isLanguage, LANGUAGE_NAMES, LANGUAGE_TAGS } from "./languages.ts";

/**
 * Pins what the environment asks for.
 *
 * `navigator.languages` is read-only, so it is defined over rather than assigned - and restored
 * afterwards, because jsdom's `navigator` is shared by every test in the file.
 */
const asksFor = (...tags: string[]) => {
    vi.stubGlobal("navigator", { ...navigator, languages: tags, language: tags[0] ?? "" });
};

afterEach(() => {
    vi.unstubAllGlobals();
});

describe("detectLanguage", () => {
    it("takes a language the application ships", () => {
        asksFor("sv-SE", "en-US");

        expect(detectLanguage()).toBe("sv");
    });

    it("falls back to English where it ships none of them", () => {
        asksFor("ko-KR", "th-TH");

        expect(detectLanguage()).toBe(DEFAULT_LANGUAGE);
    });

    it("matches a regional tag on the language itself", () => {
        // The application ships one Portuguese catalogue; a pt-BR machine gets it rather than English.
        asksFor("pt-BR");

        expect(detectLanguage()).toBe("pt");
    });

    it("matches an underscore-separated tag", () => {
        // What a webview taking its language from a POSIX locale hands over.
        asksFor("es_ES");

        expect(detectLanguage()).toBe("es");
    });

    it("walks the whole preference list rather than its first entry", () => {
        asksFor("ko-KR", "nl-BE", "en-GB");

        expect(detectLanguage()).toBe("nl");
    });

    it("reads `language` where the list is empty", () => {
        vi.stubGlobal("navigator", { ...navigator, languages: [], language: "ja-JP" });

        expect(detectLanguage()).toBe("ja");
    });
});

describe("isLanguage", () => {
    it("accepts every tag the application ships", () => {
        expect(LANGUAGE_TAGS.every(isLanguage)).toBe(true);
    });

    it.each([["ko"], ["en-GB"], ["EN"], [""], [undefined], [null], [42]])("rejects %s", (value) => {
        expect(isLanguage(value)).toBe(false);
    });
});

describe("LANGUAGE_NAMES", () => {
    it("names every language in itself", () => {
        // Against the tags rather than against a second list of names: the record's type already
        // forces one entry per tag, and what this adds is that none of them is blank - an endonym
        // that rendered as nothing would leave a row in the picker with no way to identify it.
        for (const tag of LANGUAGE_TAGS) expect(LANGUAGE_NAMES[tag].trim()).not.toBe("");
    });

    it("gives each language a distinct name", () => {
        expect(new Set(Object.values(LANGUAGE_NAMES)).size).toBe(LANGUAGE_TAGS.length);
    });
});
