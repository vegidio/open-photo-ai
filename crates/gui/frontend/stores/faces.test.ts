import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { CropInfo } from "@/ipc/crop";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useFileStore } from "@/stores/files";
import { FRAMING, HOLIDAY, resetFileStore, SUNSET } from "@/test/support";
import { useFaceChoice, useFacesStore, useImageFaces } from "./faces";

const framing = (left: number): CropInfo => ({
    left,
    top: 0,
    width: 100,
    height: 100,
    millidegrees: 0,
    flipHorizontal: false,
    flipVertical: false,
});

const face = (left: number): Face => ({
    bounding_box: { min: { x: left, y: 4 }, max: { x: left + 3, y: 7 } },
    landmarks: [
        { x: left + 1, y: 5 },
        { x: left + 2, y: 5 },
        { x: left + 1.5, y: 6 },
        { x: left + 1, y: 6.5 },
        { x: left + 2, y: 6.5 },
    ],
    confidence: 0.9,
    restorable: true,
    key: `${left},4,${left + 3},7`,
});

const identity = (record: { identity?: string }) => record.identity ?? "";

const set = (record: { identity?: string }, crop: CropInfo | undefined, faces: Face[]) =>
    act(() => useFacesStore.getState().setFaces(identity(record), crop, faces));

const held = () => useFacesStore.getState().faces;

const read = (record: { identity?: string }, crop?: CropInfo) =>
    renderHook(() => useImageFaces(identity(record), crop)).result.current;

const skip = (record: { identity?: string }, ...keys: string[]) =>
    act(() => useFacesStore.getState().setFaceChoice(identity(record), { skipped: keys, restored: [] }));

const choices = () => useFacesStore.getState().choices;

const readChoice = (record: { identity?: string }) => renderHook(() => useFaceChoice(identity(record))).result.current;

beforeEach(() => {
    resetFileStore();
    useFacesStore.setState(useFacesStore.getInitialState(), true);
});

describe("the faces store", () => {
    it("answers the faces found for the framing in force", () => {
        const at90 = framing(10);

        set(HOLIDAY, at90, [face(0), face(20)]);

        expect(read(HOLIDAY, at90)).toEqual([face(0), face(20)]);
    });

    it("answers nothing for a framing other than the one they were found at", () => {
        set(HOLIDAY, framing(10), [face(0), face(20)]);

        expect(read(HOLIDAY, framing(20))).toBeUndefined();
        expect(read(HOLIDAY, undefined)).toBeUndefined();
    });

    it("compares the framing by reference, so a framing returned to is a hit", () => {
        // A photograph turned to 90, then to 50, then back to 90 is back at the object it was detected
        // under.
        const at90 = framing(10);
        const at50 = framing(50);

        set(HOLIDAY, at90, [face(0)]);

        expect(read(HOLIDAY, at50)).toBeUndefined();
        expect(read(HOLIDAY, at90)).toEqual([face(0)]);
    });

    it("distinguishes a photograph with no faces from one whose faces are not known", () => {
        // Both are needed, and they mean different things: a detection that found nobody, and one that
        // has not happened. The first stops the run path asking again; the second is what makes it ask.
        set(HOLIDAY, undefined, []);

        expect(read(HOLIDAY, undefined)).toEqual([]);
        expect(read(SUNSET, undefined)).toBeUndefined();
    });

    it("answers nothing for a photograph with no identity", () => {
        expect(read({}, undefined)).toBeUndefined();
    });

    it("keeps one photograph's faces separate from another's", () => {
        set(HOLIDAY, undefined, [face(0)]);
        set(SUNSET, undefined, [face(40), face(80)]);

        expect(read(HOLIDAY, undefined)).toEqual([face(0)]);
        expect(read(SUNSET, undefined)).toEqual([face(40), face(80)]);
    });

    it("replaces the map on every write rather than mutating it", () => {
        const before = held();

        set(HOLIDAY, undefined, [face(0)]);

        expect(held()).not.toBe(before);
        expect(before.size).toBe(0);
    });

    it("forgets one photograph's faces and its choice when it is closed, and leaves the others alone", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, undefined, [face(0)]);
        set(SUNSET, undefined, [face(40)]);
        skip(HOLIDAY, "a");
        skip(SUNSET, "b");

        act(() => useFileStore.getState().closeFile(HOLIDAY.path));

        expect(held().has(identity(HOLIDAY))).toBe(false);
        expect(choices().has(identity(HOLIDAY))).toBe(false);
        expect(read(SUNSET, undefined)).toEqual([face(40)]);
        expect(readChoice(SUNSET)).toEqual({ skipped: ["b"], restored: [] });
    });

    it("forgets every photograph's faces and every choice when every image is closed", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, undefined, [face(0)]);
        set(SUNSET, undefined, [face(40)]);
        skip(HOLIDAY, "a");
        skip(SUNSET, "b");

        act(() => useFileStore.getState().closeAll());

        expect(held().size).toBe(0);
        expect(choices().size).toBe(0);
    });

    it("writes nothing outside itself", () => {
        set(HOLIDAY, undefined, [face(0)]);

        expect(localStorage.length).toBe(0);
        expect(sessionStorage.length).toBe(0);
    });
});

describe("recording what a detection found", () => {
    it("records the faces and writes no choice, whatever their sizes", () => {
        // The default among them is each face's own `restorable`, so a detection decides nothing: that is what lets
        // the choice be a dependency of the run without a detection landing re-running it.
        const large: Face = { ...face(0), restorable: false };

        set(HOLIDAY, undefined, [large, face(20)]);

        expect(read(HOLIDAY, undefined)).toEqual([large, face(20)]);
        expect(choices().size).toBe(0);
    });

    it("leaves the choice exactly as it was when the faces are found again", () => {
        skip(HOLIDAY, faceKey(face(0)));
        const before = choices();

        set(HOLIDAY, undefined, [face(0), face(20)]);
        set(HOLIDAY, FRAMING, [face(40)]);

        expect(choices()).toBe(before);
    });
});

describe("the choice made among a photograph's faces", () => {
    it("answers nothing for a photograph nobody has chosen in", () => {
        expect(readChoice(HOLIDAY)).toBeUndefined();
    });

    it("answers nothing for a photograph with no identity", () => {
        expect(renderHook(() => useFaceChoice(undefined)).result.current).toBeUndefined();
    });

    it("answers the exceptions that were recorded", () => {
        act(() => useFacesStore.getState().setFaceChoice(identity(HOLIDAY), { skipped: ["a"], restored: ["b"] }));

        expect(readChoice(HOLIDAY)).toEqual({ skipped: ["a"], restored: ["b"] });
    });

    it("keeps one photograph's choice separate from another's", () => {
        skip(HOLIDAY, "a");
        skip(SUNSET, "b");

        expect(readChoice(HOLIDAY)).toEqual({ skipped: ["a"], restored: [] });
        expect(readChoice(SUNSET)).toEqual({ skipped: ["b"], restored: [] });
    });

    it("replaces the map and the choice on every write rather than mutating them", () => {
        skip(HOLIDAY, "a");
        const map = choices();
        const choice = map.get(identity(HOLIDAY));

        skip(HOLIDAY, "a");

        expect(choices()).not.toBe(map);
        expect(choices().get(identity(HOLIDAY))).not.toBe(choice);
    });

    it("copies the lists it is handed, so the caller's working copy is not what is committed", () => {
        const skipped = ["a"];
        act(() => useFacesStore.getState().setFaceChoice(identity(HOLIDAY), { skipped, restored: [] }));

        skipped.push("b");

        expect(readChoice(HOLIDAY)).toEqual({ skipped: ["a"], restored: [] });
    });

    it("hands back the same reference until a write happens", () => {
        skip(HOLIDAY, "a");
        const { result, rerender } = renderHook(() => useFaceChoice(identity(HOLIDAY)));
        const first = result.current;

        rerender();
        set(HOLIDAY, undefined, [face(0)]);
        rerender();

        expect(result.current).toBe(first);
    });
});
