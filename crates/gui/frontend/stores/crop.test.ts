import { act } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { CropInfo } from "@/ipc/crop";
import { useFileStore } from "@/stores/files";
import { HOLIDAY, resetFileStore, SUNSET } from "@/test/support";
import { useCropStore } from "./crop";

const framing = (left: number): CropInfo => ({
    left,
    top: 0,
    width: 100,
    height: 100,
    millidegrees: 0,
    flipHorizontal: false,
    flipVertical: false,
});

const identity = (record: { identity?: string }) => record.identity ?? "";

const set = (record: { identity?: string }, crop: CropInfo) =>
    act(() => useCropStore.getState().setCrop(identity(record), crop));

const crops = () => useCropStore.getState().crops;

beforeEach(() => {
    resetFileStore();
    useCropStore.setState(useCropStore.getInitialState(), true);
});

describe("the crop store", () => {
    it("keeps one photograph's framing separate from another's", () => {
        set(HOLIDAY, framing(10));
        set(SUNSET, framing(20));

        expect(crops().get(identity(HOLIDAY))).toEqual(framing(10));
        expect(crops().get(identity(SUNSET))).toEqual(framing(20));
    });

    it("replaces the map on every write rather than mutating it", () => {
        const before = crops();

        set(HOLIDAY, framing(10));

        // The standing rule, argued in `setTransform` in `stores/transform.ts`.
        expect(crops()).not.toBe(before);
        expect(before.size).toBe(0);
    });

    it("keeps a framing while the photograph it belongs to stays open", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, framing(10));

        // Moving between images is not closing one: the framing of the photograph left behind is
        // still there when the user comes back to it.
        act(() => useFileStore.getState().setCurrentIndex(1));
        act(() => useFileStore.getState().setCurrentIndex(0));

        expect(crops().get(identity(HOLIDAY))).toEqual(framing(10));
    });

    it("forgets one photograph's framing when it is closed and leaves the others alone", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, framing(10));
        set(SUNSET, framing(20));

        act(() => useFileStore.getState().closeFile(HOLIDAY.path));

        // Registered as a file owner rather than called by name, which is what makes this the file
        // store's own close path rather than a second thing a caller has to remember.
        expect(crops().has(identity(HOLIDAY))).toBe(false);
        expect(crops().get(identity(SUNSET))).toEqual(framing(20));
    });

    it("forgets every framing when every image is closed", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, framing(10));
        set(SUNSET, framing(20));

        act(() => useFileStore.getState().closeAll());

        expect(crops().size).toBe(0);
    });

    it("writes nothing outside itself", () => {
        // A persisted store writes through to storage on every set, so an untouched storage after a
        // write is what says there is no middleware here.
        set(HOLIDAY, framing(10));

        expect(localStorage.length).toBe(0);
        expect(sessionStorage.length).toBe(0);
    });
});
