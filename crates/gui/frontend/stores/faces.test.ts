import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { CropInfo } from "@/ipc/crop";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useFileStore } from "@/stores/files";
import { HOLIDAY, resetFileStore, SUNSET } from "@/test/support";
import { useFacesStore, useImageFaces, useSkippedFaces } from "./faces";

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
});

const identity = (record: { identity?: string }) => record.identity ?? "";

const set = (record: { identity?: string }, crop: CropInfo | undefined, faces: Face[]) =>
    act(() => useFacesStore.getState().setFaces(identity(record), crop, faces));

const held = () => useFacesStore.getState().faces;

const read = (record: { identity?: string }, crop?: CropInfo) =>
    renderHook(() => useImageFaces(identity(record), crop)).result.current;

const skip = (record: { identity?: string }, ...keys: string[]) =>
    act(() => useFacesStore.getState().setSkippedFaces(identity(record), new Set(keys)));

const skipped = () => useFacesStore.getState().skipped;

const decided = () => useFacesStore.getState().decided;

const readSkipped = (record: { identity?: string }) =>
    renderHook(() => useSkippedFaces(identity(record))).result.current;

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
        expect(skipped().has(identity(HOLIDAY))).toBe(false);
        expect(decided().has(identity(HOLIDAY))).toBe(false);
        expect(read(SUNSET, undefined)).toEqual([face(40)]);
        expect(readSkipped(SUNSET)).toEqual(new Set(["b"]));
        expect(decided().has(identity(SUNSET))).toBe(true);
    });

    it("forgets every photograph's faces and every choice when every image is closed", () => {
        act(() => useFileStore.getState().addFiles([HOLIDAY, SUNSET]));
        set(HOLIDAY, undefined, [face(0)]);
        set(SUNSET, undefined, [face(40)]);
        skip(HOLIDAY, "a");
        skip(SUNSET, "b");

        act(() => useFileStore.getState().closeAll());

        expect(held().size).toBe(0);
        expect(skipped().size).toBe(0);
        expect(decided().size).toBe(0);
    });

    it("writes nothing outside itself", () => {
        set(HOLIDAY, undefined, [face(0)]);

        expect(localStorage.length).toBe(0);
        expect(sessionStorage.length).toBe(0);
    });
});

/** A face `edge` pixels square, at `left`: the size rule looks at nothing else. */
const square = (left: number, edge: number): Face => ({
    ...face(left),
    bounding_box: { min: { x: left, y: 0 }, max: { x: left + edge, y: edge } },
});

/** A face the size rule leaves chosen, and one it skips. */
const restorable = square(0, 512);
const oversized = square(1000, 513);

describe("the choice a detection is recorded with", () => {
    it("skips a face too large to be worth restoring and chooses the rest", () => {
        set(HOLIDAY, undefined, [face(0), oversized, restorable]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(oversized)]));
    });

    it("holds no choice at all for a photograph whose faces are all restorable", () => {
        // An absent set rather than an empty one: `enabledFaces` answers both by handing the run the
        // array it was given, and the absent one is the one that costs nothing to hold.
        set(HOLIDAY, undefined, [face(0), restorable]);

        expect(skipped().has(identity(HOLIDAY))).toBe(false);
    });

    it("leaves a face it has already decided about as the user left it", () => {
        // A re-detection at the framing in force - the same crop object handed back, or a run asking
        // again - must not undo a choice the user made since.
        const at90 = framing(10);

        set(HOLIDAY, at90, [oversized]);
        skip(HOLIDAY);

        set(HOLIDAY, at90, [oversized]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set());
    });

    it("writes no new set when it decides nothing, so no run is started", () => {
        const at90 = framing(10);

        set(HOLIDAY, at90, [oversized]);
        const before = readSkipped(HOLIDAY);

        set(HOLIDAY, at90, [oversized]);

        expect(readSkipped(HOLIDAY)).toBe(before);
    });

    it("decides the faces found at a new framing afresh", () => {
        // Every face moved, so nothing was decided before: the default that applies when the old
        // choice stops matching is the size rule rather than "everything chosen".
        const moved = square(2000, 513);

        set(HOLIDAY, framing(10), [oversized]);
        set(HOLIDAY, framing(40), [moved]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(oversized), faceKey(moved)]));
    });

    it("leaves a face the user turned on alone when the framing it was turned on at is returned to", () => {
        // The framing returned to, from the other side: `gui-faces` requires that a face the user has
        // decided keeps what the user made of it, and the size rule may not undo a choice made by
        // hand. Reading "already decided" off the faces this write replaces - one framing's worth -
        // makes the big face undecided the moment another framing has been visited, and skips it
        // again behind the user's back on the way home.
        const moved = square(2000, 513);
        const home = framing(10);

        set(HOLIDAY, home, [oversized]);
        skip(HOLIDAY);

        set(HOLIDAY, framing(40), [moved]);
        set(HOLIDAY, home, [oversized]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(moved)]));
    });

    it("leaves a face the user skipped by hand skipped when its framing is returned to", () => {
        // And the same round trip for a small face, which the size rule would never have skipped: the
        // choice comes back because the key does, not because anything carried it.
        const home = framing(10);

        set(HOLIDAY, home, [restorable]);
        skip(HOLIDAY, faceKey(restorable));

        set(HOLIDAY, framing(40), [square(2000, 100)]);
        set(HOLIDAY, home, [restorable]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(restorable)]));
    });

    it("records the faces and the choice among them in one write", () => {
        let renders = 0;

        const { result } = renderHook(() => {
            renders += 1;

            return [useImageFaces(identity(HOLIDAY), undefined), useSkippedFaces(identity(HOLIDAY))] as const;
        });

        const before = renders;
        set(HOLIDAY, undefined, [oversized, restorable]);

        expect(renders).toBe(before + 1);
        expect(result.current[0]).toEqual([oversized, restorable]);
        expect(result.current[1]).toEqual(new Set([faceKey(oversized)]));
    });

    it("keeps one photograph's decision separate from another's", () => {
        set(HOLIDAY, undefined, [oversized]);
        set(SUNSET, undefined, [restorable]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(oversized)]));
        expect(readSkipped(SUNSET)).toBeUndefined();
    });

    it("decides them again when the photograph is closed and opened", () => {
        set(HOLIDAY, undefined, [oversized]);
        skip(HOLIDAY);

        act(() => useFacesStore.getState().forgetFaces(identity(HOLIDAY)));
        set(HOLIDAY, undefined, [oversized]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set([faceKey(oversized)]));
    });
});

describe("the choice made among a photograph's faces", () => {
    it("answers nothing for a photograph nothing has been skipped in", () => {
        set(HOLIDAY, undefined, [face(0), face(20)]);

        expect(readSkipped(HOLIDAY)).toBeUndefined();
    });

    it("answers nothing for a photograph with no identity", () => {
        expect(readSkipped({})).toBeUndefined();
    });

    it("answers the keys that were skipped", () => {
        skip(HOLIDAY, "a", "b");

        expect(readSkipped(HOLIDAY)).toEqual(new Set(["a", "b"]));
    });

    it("keeps one photograph's choice separate from another's", () => {
        skip(HOLIDAY, "a");
        skip(SUNSET, "b");

        expect(readSkipped(HOLIDAY)).toEqual(new Set(["a"]));
        expect(readSkipped(SUNSET)).toEqual(new Set(["b"]));
    });

    it("records that nothing is skipped as an empty set rather than as no entry", () => {
        skip(HOLIDAY, "a");
        skip(HOLIDAY);

        expect(readSkipped(HOLIDAY)).toEqual(new Set());
        expect(skipped().has(identity(HOLIDAY))).toBe(true);
    });

    it("survives a re-detection at the same framing", () => {
        // Faces and the choice among them are written by different paths: a detection landing again
        // must not clear a choice, or a re-render that re-detected would silently restore every face.
        const at90 = framing(10);

        set(HOLIDAY, at90, [face(0), face(20)]);
        skip(HOLIDAY, "a");

        set(HOLIDAY, at90, [face(0), face(20)]);

        expect(readSkipped(HOLIDAY)).toEqual(new Set(["a"]));
    });

    it("replaces the map and the set on every write rather than mutating them", () => {
        // What `useEnhancementRun` is built on: the effect lists the set, so a set mutated in place
        // would never re-run the chain, and a map mutated in place would never re-render at all.
        skip(HOLIDAY, "a");

        const map = skipped();
        const before = map.get(identity(HOLIDAY));

        skip(HOLIDAY, "a", "b");

        expect(skipped()).not.toBe(map);
        expect(skipped().get(identity(HOLIDAY))).not.toBe(before);
        expect(before).toEqual(new Set(["a"]));
    });

    it("copies the set it is handed, so the caller's working copy is not what is committed", () => {
        // The dialog edits a `Set` it goes on holding. Storing that object would make every later
        // toggle a silent write to the store, with nothing re-rendering and no run asked for.
        const working = new Set(["a"]);

        act(() => useFacesStore.getState().setSkippedFaces(identity(HOLIDAY), working));
        working.add("b");

        expect(readSkipped(HOLIDAY)).toEqual(new Set(["a"]));
    });

    it("hands back the same reference until a write happens", () => {
        skip(HOLIDAY, "a");

        expect(readSkipped(HOLIDAY)).toBe(readSkipped(HOLIDAY));
    });
});
