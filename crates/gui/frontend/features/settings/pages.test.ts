import { describe, expect, it } from "vitest";
import { settingsDefaults } from "@/stores/settings";
import { SETTINGS_PAGES } from "./pages.ts";

describe("the settings pages", () => {
    const placed = SETTINGS_PAGES.flatMap((page) => page.keys);

    it("put no preference on two pages", () => {
        // One changed in one place and read back in another. The type check covers the other half - a
        // preference on no page - which a test could only assert against a list it wrote itself.
        expect(new Set(placed).size).toBe(placed.length);
    });

    it("place every preference the store holds", () => {
        // The runtime twin of `_EveryPreferenceIsOnAPage`, against the store's own defaults.
        expect([...placed].sort()).toEqual(Object.keys(settingsDefaults()).sort());
    });
});
