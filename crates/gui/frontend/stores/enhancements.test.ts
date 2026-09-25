import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { type Operation, releaseAllEnhanced, releaseEnhanced } from "@/ipc/enhance";
import type { ImageRecord } from "@/ipc/images";
import { HOLIDAY, openFiles, resetFileStore, SUNSET } from "@/test/support";
import { useEnhancementStore } from "./enhancements";
import { useFileStore } from "./files";

// Only the two releases are replaced: the rest of the module is what every other store and test in
// this file already reads, and the point here is what closing a file asks Rust to let go of.
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

const released = releaseEnhanced as unknown as Mock;
const releasedAll = releaseAllEnhanced as unknown as Mock;

// The key the Wails app used, asserted here rather than imported: a rename would keep this test
// passing if it read the value from the module, while silently discarding every existing user's
// setting.
const STORAGE_KEY = "enhancements-storage";

describe("useEnhancementStore", () => {
    beforeEach(() => {
        localStorage.clear();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    });

    it("is on before anyone has chosen", () => {
        expect(useEnhancementStore.getState().autopilot).toBe(true);
    });

    it("toggles", () => {
        useEnhancementStore.getState().toggle();

        expect(useEnhancementStore.getState().autopilot).toBe(false);
    });

    it("comes up off when a persisted `false` is rehydrated", async () => {
        // The shape `persist` writes, seeded by hand: this is what the previous run left behind, so
        // rehydrating it is what a restart does.
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ state: { autopilot: false }, version: 0 }));

        await useEnhancementStore.persist.rehydrate();

        expect(useEnhancementStore.getState().autopilot).toBe(false);
    });
});

/** An upscale carrying whichever model and scale a case is about. */
const upscale = (codename: string, scale: number): Operation => ({
    family: "upscale",
    codename,
    precision: "fp32",
    scale,
});

/** What one image is set to have done to it, read straight off the store. */
const stackOf = (path: string) => useEnhancementStore.getState().enhancements.get(path);

describe("the per-image enhancement stack", () => {
    beforeEach(() => {
        localStorage.clear();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        resetFileStore();
    });

    it("starts every image with nothing", () => {
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
    });

    it("adds an enhancement to the image it names", () => {
        useEnhancementStore.getState().addEnhancement(HOLIDAY.path, upscale("kyoto", 2));

        expect(stackOf(HOLIDAY.path)).toEqual([upscale("kyoto", 2)]);
    });

    it("keeps the stack in the order the enhancements are applied", () => {
        const { addEnhancement } = useEnhancementStore.getState();

        // Upscale added first, and `ENHANCEMENTS` runs it last of the seven - so the order the user
        // added them in is not the order the chain runs in, and the list is the chain.
        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        addEnhancement(HOLIDAY.path, { family: "face_recovery", codename: "athens", precision: "fp32", faces: [] });

        expect(stackOf(HOLIDAY.path)?.map((operation) => operation.family)).toEqual(["face_recovery", "upscale"]);
    });

    it("replaces the enhancement of that family and leaves the array's length alone", () => {
        const { addEnhancement, replaceEnhancement } = useEnhancementStore.getState();

        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        replaceEnhancement(HOLIDAY.path, upscale("osaka", 4));

        expect(stackOf(HOLIDAY.path)).toEqual([upscale("osaka", 4)]);
    });

    it("leaves an image with no stack alone when one is replaced on it", () => {
        useEnhancementStore.getState().replaceEnhancement(HOLIDAY.path, upscale("osaka", 4));

        expect(stackOf(HOLIDAY.path)).toBeUndefined();
    });

    it("removes the enhancement of that family", () => {
        const { addEnhancement, removeEnhancement } = useEnhancementStore.getState();

        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        removeEnhancement(HOLIDAY.path, "upscale");

        expect(stackOf(HOLIDAY.path)).toEqual([]);
    });

    it("gives two images stacks of their own", () => {
        const { addEnhancement } = useEnhancementStore.getState();

        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        addEnhancement(SUNSET.path, upscale("tokyo", 1));

        expect(stackOf(HOLIDAY.path)).toEqual([upscale("kyoto", 2)]);
        expect(stackOf(SUNSET.path)).toEqual([upscale("tokyo", 1)]);
    });

    it("discards a closed image's stack and leaves every other image's alone", () => {
        openFiles(HOLIDAY, SUNSET);

        const { addEnhancement } = useEnhancementStore.getState();
        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        addEnhancement(SUNSET.path, upscale("tokyo", 1));

        useFileStore.getState().closeFile(HOLIDAY.path);

        // And opening the same file again therefore starts it with nothing, which is the whole of
        // what "a stack is about the photograph on screen now" means.
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
        expect(stackOf(SUNSET.path)).toEqual([upscale("tokyo", 1)]);
    });

    it("discards every stack when every image is closed", () => {
        openFiles(HOLIDAY, SUNSET);

        const { addEnhancement } = useEnhancementStore.getState();
        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        addEnhancement(SUNSET.path, upscale("tokyo", 1));

        useFileStore.getState().closeAll();

        expect(useEnhancementStore.getState().enhancements.size).toBe(0);
    });

    it("replaces the Map rather than mutating it, so a subscriber sees the write", () => {
        const before = useEnhancementStore.getState().enhancements;

        useEnhancementStore.getState().addEnhancement(HOLIDAY.path, upscale("kyoto", 2));

        expect(useEnhancementStore.getState().enhancements).not.toBe(before);
    });

    it("persists autopilot and nothing else", () => {
        useEnhancementStore.getState().addEnhancement(HOLIDAY.path, upscale("kyoto", 2));

        const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as { state?: object };

        expect(stored.state).toEqual({ autopilot: true });
    });
});

describe("a batch of enhancements", () => {
    const light: Operation = { family: "light_adjustment", codename: "paris", precision: "fp32", bias: 0.5 };
    const recovery: Operation = { family: "face_recovery", codename: "athens", precision: "fp32", faces: [] };

    beforeEach(() => {
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    });

    it("lands in the order the enhancements are applied", () => {
        useEnhancementStore.getState().addEnhancements(HOLIDAY.path, [upscale("kyoto", 4), light, recovery]);

        expect(stackOf(HOLIDAY.path)?.map((operation) => operation.family)).toEqual([
            "face_recovery",
            "light_adjustment",
            "upscale",
        ]);
    });

    it("keeps the enhancement already in the stack for a family the batch also carries", () => {
        const { addEnhancement, addEnhancements } = useEnhancementStore.getState();

        // Added by hand while the analysis ran: the user's is the later word.
        addEnhancement(HOLIDAY.path, upscale("kyoto", 2));
        addEnhancements(HOLIDAY.path, [light, upscale("tokyo", 4)]);

        expect(stackOf(HOLIDAY.path)).toEqual([light, upscale("kyoto", 2)]);
    });

    it("gives an image a stack even when the batch is empty", () => {
        useEnhancementStore.getState().addEnhancements(HOLIDAY.path, []);

        // Which is what makes an analysis that answered nothing mean "analysed".
        expect(useEnhancementStore.getState().enhancements.has(HOLIDAY.path)).toBe(true);
        expect(stackOf(HOLIDAY.path)).toEqual([]);
    });

    it("is one write to the store however many it carries", () => {
        const heard = vi.fn();
        const unsubscribe = useEnhancementStore.subscribe(heard);

        useEnhancementStore.getState().addEnhancements(HOLIDAY.path, [upscale("kyoto", 4), light, recovery]);
        unsubscribe();

        expect(heard).toHaveBeenCalledTimes(1);
    });
});

/**
 * A photograph whose bytes could not be read, which is the one open image with no identity.
 *
 * Spelled by omission rather than by an explicit `undefined`, which `exactOptionalPropertyTypes`
 * refuses: the property is absent on the record, not present and empty.
 */
const UNREADABLE: ImageRecord = {
    path: "/Users/someone/Pictures/corrupt.png",
    width: 0,
    height: 0,
    extension: "png",
    size: 12,
};

describe("releasing what Rust is holding", () => {
    beforeEach(() => {
        localStorage.clear();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        resetFileStore();
        released.mockReset().mockResolvedValue(undefined);
        releasedAll.mockReset().mockResolvedValue(undefined);
    });

    it("releases the closed photograph's own identity and nothing else", () => {
        openFiles(HOLIDAY, SUNSET);

        useFileStore.getState().closeFile(HOLIDAY.path);

        // The identity on the record being removed, because the backend matches a release against the
        // photograph a result was made from rather than against the result. A release naming SUNSET
        // would throw away the result the window may still be drawing.
        expect(released).toHaveBeenCalledTimes(1);
        expect(released).toHaveBeenCalledWith(HOLIDAY.identity);
        expect(releasedAll).not.toHaveBeenCalled();
    });

    it("releases nothing for a photograph that has no identity", () => {
        // A file whose bytes could not be read was never a source, so nothing was ever enhanced from
        // it - and a release naming nothing at all is what the named release exists to avoid.
        openFiles(UNREADABLE);

        useFileStore.getState().closeFile(UNREADABLE.path);

        expect(released).not.toHaveBeenCalled();
        expect(releasedAll).not.toHaveBeenCalled();
    });

    it("releases everything when every photograph is closed", () => {
        openFiles(HOLIDAY, SUNSET);

        useFileStore.getState().closeAll();

        expect(releasedAll).toHaveBeenCalledTimes(1);
        expect(released).not.toHaveBeenCalled();
    });

    it("keeps a refused release away from the user", async () => {
        // Through `report`, which writes every failure it is given to the console as an error.
        const warned = vi.spyOn(console, "error").mockImplementation(() => {});
        released.mockRejectedValueOnce(new Error("the window is gone"));

        openFiles(HOLIDAY);

        expect(() => useFileStore.getState().closeFile(HOLIDAY.path)).not.toThrow();

        // The close still did its own work, which is the half the user would actually notice.
        expect(useFileStore.getState().files).toEqual([]);

        await Promise.resolve();
        expect(warned).toHaveBeenCalled();

        warned.mockRestore();
    });
});
