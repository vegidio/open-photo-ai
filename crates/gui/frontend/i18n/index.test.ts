import { describe, expect, it } from "vitest";
import i18n, { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES } from "./index.ts";

describe("i18n", () => {
    it("initialises to English, which is what this environment asks for", () => {
        expect(i18n.resolvedLanguage).toBe(DEFAULT_LANGUAGE);
    });

    it("resolves a key from the ported catalogue", () => {
        expect(i18n.t("preview.empty.browse")).toBe("Browse images");
    });

    it("ships the thirteen languages the Wails app had", () => {
        expect([...SUPPORTED_LANGUAGES].sort()).toEqual([
            "de",
            "el",
            "en",
            "es",
            "fr",
            "hi",
            "id",
            "ja",
            "nl",
            "pt",
            "ru",
            "sv",
            "zh",
        ]);
    });

    it("serves every language it claims to support", async () => {
        for (const language of SUPPORTED_LANGUAGES) {
            await i18n.changeLanguage(language);
            expect(i18n.resolvedLanguage).toBe(language);
            // A language that fell through to English would still return a string, so the assertion
            // is against the fallback rather than against emptiness.
            expect(i18n.t("preview.empty.browse")).not.toBe("");
        }

        await i18n.changeLanguage(DEFAULT_LANGUAGE);
    });
});
