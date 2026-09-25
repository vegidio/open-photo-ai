import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Which language the application actually comes up in.
 *
 * Its own file, and the imports are dynamic, because that is the whole thing under test: `i18n`
 * reads the store's language once, as it initialises, and the store reads `localStorage` once, as it
 * is created. Both happen at import. A test importing either at the top of the file would have
 * settled the question before it could seed anything, and would then be asserting on whatever the
 * environment happened to be.
 *
 * This is the check behind a decision that makes initialization order load-bearing - see the comment
 * on `lng` in `index.ts`. If `persist` ever stopped rehydrating synchronously, this is what would
 * say so.
 */
beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
});

describe("the language the application comes up in", () => {
    it("is the one the user chose on a previous run", async () => {
        localStorage.setItem("settings-storage", JSON.stringify({ state: { language: "ja" }, version: 0 }));

        const { default: i18n } = await import("./index.ts");

        expect(i18n.resolvedLanguage).toBe("ja");
        // Not merely selected: the catalogue behind it is the one being served.
        expect(i18n.t("common.save")).not.toBe("Save");
    });

    it("is the one this machine asks for, where nothing was chosen", async () => {
        vi.stubGlobal("navigator", { ...navigator, languages: ["ko-KR", "de-AT"], language: "ko-KR" });

        const { default: i18n } = await import("./index.ts");

        // The second preference, because the first is a language the application ships no catalogue
        // for - which is the reason detection walks the list rather than reading its first entry.
        expect(i18n.resolvedLanguage).toBe("de");

        vi.unstubAllGlobals();
    });

    it("is English where it ships none of the languages this machine asks for", async () => {
        vi.stubGlobal("navigator", { ...navigator, languages: ["ko-KR"], language: "ko-KR" });

        const { default: i18n } = await import("./index.ts");

        expect(i18n.resolvedLanguage).toBe("en");

        vi.unstubAllGlobals();
    });
});
