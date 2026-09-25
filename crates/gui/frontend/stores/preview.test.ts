import { beforeEach, describe, expect, it } from "vitest";
import { usePreviewStore } from "./preview";

// The key the Wails app used, asserted here rather than imported: a rename would keep this test
// passing if it read the value from the module, while silently discarding every existing user's
// choice. The same argument `enhancements.test.ts` makes for its own key.
const STORAGE_KEY = "app-storage";

describe("usePreviewStore", () => {
    beforeEach(() => {
        localStorage.clear();
        usePreviewStore.setState({ previewMode: "side" });
    });

    it("compares side by side before anyone has chosen", () => {
        expect(usePreviewStore.getState().previewMode).toBe("side");
    });

    it("draws whichever comparison is chosen", () => {
        usePreviewStore.getState().setPreviewMode("split");

        expect(usePreviewStore.getState().previewMode).toBe("split");
    });

    it("writes the choice under the key the Wails app wrote, spelled as that app spelled it", async () => {
        usePreviewStore.getState().setPreviewMode("full");

        // `persist` writes on a microtask, so the read is awaited rather than taken on the next line.
        await Promise.resolve();

        expect(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null")).toEqual({
            state: { previewMode: "full" },
            version: 0,
        });
    });

    it("comes up on the comparison the previous run was left in", async () => {
        // The shape `persist` writes, seeded by hand: this is what the Wails application left behind,
        // so rehydrating it is what an upgrade does.
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ state: { previewMode: "split" }, version: 0 }));

        await usePreviewStore.persist.rehydrate();

        expect(usePreviewStore.getState().previewMode).toBe("split");
    });
});
