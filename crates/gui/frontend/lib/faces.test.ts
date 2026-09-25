import { describe, expect, it } from "vitest";
import type { Operation } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import {
    decidedSkips,
    enabledFaces,
    faceKey,
    noFaceToRestore,
    RESTORED_FACE_AREA,
    withDecided,
    withFaces,
} from "./faces";

const at = (left: number, top = 4): Face => ({
    bounding_box: { min: { x: left, y: top }, max: { x: left + 3, y: top + 3 } },
    landmarks: [
        { x: left + 1, y: top + 1 },
        { x: left + 2, y: top + 1 },
        { x: left + 1.5, y: top + 2 },
        { x: left + 1, y: top + 2.5 },
        { x: left + 2, y: top + 2.5 },
    ],
    confidence: 0.9,
});

describe("a face's key", () => {
    it("is the four bounding-box coordinates", () => {
        expect(faceKey(at(10))).toBe("10,4,13,7");
    });

    it("is equal for two faces carrying the same bounding box", () => {
        // Which is what a re-detection of one photograph at one framing produces: `opai` quantizes a
        // coordinate to a hundredth of a pixel on acceptance, so the numbers are the same numbers.
        const found = at(10);
        const again: Face = { ...at(10), confidence: 0.7 };

        expect(faceKey(again)).toBe(faceKey(found));
    });

    it("differs for two faces carrying different bounding boxes", () => {
        expect(faceKey(at(10))).not.toBe(faceKey(at(20)));
        expect(faceKey(at(10))).not.toBe(faceKey(at(10, 5)));
    });

    it("tells a fractional coordinate from a whole one", () => {
        const shifted: Face = { ...at(10), bounding_box: { min: { x: 10.25, y: 4 }, max: { x: 13, y: 7 } } };

        expect(faceKey(shifted)).toBe("10.25,4,13,7");
        expect(faceKey(shifted)).not.toBe(faceKey(at(10)));
    });
});

describe("the faces a run restores", () => {
    it("is the array itself when nothing is skipped", () => {
        // Load-bearing rather than a saving: `useEnhancementRun` lists the faces among its effect's
        // dependencies, so a fresh array would cancel the run in flight on every render.
        const found = [at(10), at(20)];

        expect(enabledFaces(found, undefined)).toBe(found);
        expect(enabledFaces(found, new Set())).toBe(found);
    });

    it("leaves out the faces whose keys were skipped", () => {
        const found = [at(10), at(20)];

        expect(enabledFaces(found, new Set([faceKey(at(10))]))).toEqual([at(20)]);
    });

    it("leaves every face in when a skipped key matches none of them", () => {
        // What a framing change comes to: every face is at new coordinates, so the keys recorded at
        // the old framing match nothing and every face in the new one is chosen.
        const found = [at(10), at(20)];

        expect(enabledFaces(found, new Set(["999,999,999,999"]))).toEqual(found);
    });

    it("preserves the order of the faces that survive", () => {
        // Folded into the run cache tag of the recovery they are handed to, so a reordering would ask
        // for a different result for the same selection.
        const found = [at(10), at(20), at(30)];

        expect(enabledFaces(found, new Set([faceKey(at(20))]))).toEqual([at(10), at(30)]);
    });

    it("answers nothing at all when every face is skipped", () => {
        const found = [at(10), at(20)];

        expect(enabledFaces(found, new Set(found.map(faceKey)))).toEqual([]);
    });

    it("answers an empty list for a photograph with nobody in it", () => {
        expect(enabledFaces([], new Set(["10,4,13,7"]))).toEqual([]);
    });
});

/** A face `edge` pixels square, at `left`, which is the only thing the size rule looks at. */
const square = (left: number, edge: number): Face => ({
    ...at(left),
    bounding_box: { min: { x: left, y: 0 }, max: { x: left + edge, y: edge } },
});

/** A face the size rule leaves chosen, and one it skips. */
const small = square(0, 512);
const large = square(1000, 513);

describe("the choice a detection is decided into", () => {
    it("skips a face larger than the tile the recovery models run at", () => {
        expect(decidedSkips([large], undefined, undefined)).toEqual(new Set([faceKey(large)]));
    });

    it("chooses a face smaller than the tile", () => {
        expect(decidedSkips([square(0, 511)], undefined, undefined)).toBeUndefined();
    });

    it("chooses a face exactly at the tile, which is neither reduced nor enlarged", () => {
        expect(RESTORED_FACE_AREA).toBe(512 * 512);
        expect(decidedSkips([small], undefined, undefined)).toBeUndefined();
    });

    it("measures the area rather than either edge", () => {
        // A long, shallow box covering fewer pixels than the tile is restorable, and a squarer one
        // covering more is not - which an edge-by-edge rule would get the wrong way round.
        const shallow: Face = { ...at(0), bounding_box: { min: { x: 0, y: 0 }, max: { x: 4096, y: 63 } } };
        const squarish: Face = { ...at(0), bounding_box: { min: { x: 0, y: 0 }, max: { x: 600, y: 500 } } };

        expect(decidedSkips([shallow], undefined, undefined)).toBeUndefined();
        expect(decidedSkips([squarish], undefined, undefined)).toEqual(new Set([faceKey(squarish)]));
    });

    it("decides each face on its own size", () => {
        expect(decidedSkips([small, large], undefined, undefined)).toEqual(new Set([faceKey(large)]));
    });

    it("leaves a face it has already decided about exactly as it is", () => {
        // A re-detection at the framing in force: the user turned the big face on, and finding it
        // again where it already was must not turn it back off.
        expect(decidedSkips([large], new Set([faceKey(large)]), new Set())).toEqual(new Set());
    });

    it("leaves a face it decided at a framing that has since been left alone", () => {
        // The framing returned to. `decided` is every face the photograph has ever been asked about,
        // not the faces of whichever detection happened last, so the big face the user turned on at
        // this framing is still decided when the user crops away and comes back - and is not skipped
        // again behind their back. Read against the last detection alone this is the bug.
        const elsewhere = square(2000, 513);
        const seen = new Set([faceKey(large), faceKey(elsewhere)]);

        expect(decidedSkips([large], seen, new Set([faceKey(elsewhere)]))).toEqual(new Set([faceKey(elsewhere)]));
    });

    it("decides a face found where none was before", () => {
        // Which is what a framing nobody has been at comes to - every face is at new coordinates - and
        // what the repository owner chose: the default that applies when the old choice is dropped is
        // this rule.
        const moved = square(2000, 513);

        expect(decidedSkips([moved], new Set([faceKey(large)]), new Set([faceKey(large)]))).toEqual(
            new Set([faceKey(large), faceKey(moved)]),
        );
    });

    it("never turns a skipped face back on", () => {
        // A small face the user skipped by hand stays skipped when it is found again, and a crop
        // elsewhere in the photograph does not restore what the user asked to be left alone.
        const chosen = new Set([faceKey(small)]);

        expect(decidedSkips([small], undefined, chosen)).toBe(chosen);
    });

    it("hands back the set it was given when it adds nothing", () => {
        // Load-bearing: the store writes only what this changes, and a set replaced with an equal one
        // is what `useEnhancementRun` reads as a choice the user just applied.
        const committed = new Set(["a"]);

        expect(decidedSkips([small], undefined, committed)).toBe(committed);
        expect(decidedSkips([], undefined, committed)).toBe(committed);
        expect(decidedSkips([], undefined, undefined)).toBeUndefined();
    });

    it("adds nothing twice", () => {
        const once = decidedSkips([large], undefined, undefined);

        expect(decidedSkips([large], undefined, once)).toBe(once);
    });

    it("answers nothing for a photograph with nobody in it", () => {
        expect(decidedSkips([], undefined, undefined)).toBeUndefined();
    });
});

describe("the faces a photograph has been asked about", () => {
    it("is every face found in it", () => {
        expect(withDecided([small, large], undefined)).toEqual(new Set([faceKey(small), faceKey(large)]));
    });

    it("keeps the faces found at every framing visited, not only the last", () => {
        // The whole of what gives a framing returned to back what the user chose there: the keys from
        // the framing they cropped away from are still in here when they crop back to it.
        const moved = square(2000, 513);
        const first = withDecided([large], undefined);

        expect(withDecided([moved], first)).toEqual(new Set([faceKey(large), faceKey(moved)]));
    });

    it("hands back the set it was given when every face found is already in it", () => {
        // Which is what a re-detection at the framing in force is, and it must write nothing: the
        // store replaces the map only where this changes, as it does for the choice beside it.
        const seen = withDecided([small, large], undefined);

        expect(withDecided([small, large], seen)).toBe(seen);
        expect(withDecided([], seen)).toBe(seen);
        expect(withDecided([], undefined)).toBeUndefined();
    });

    it("adds a face only once however often it is found", () => {
        const seen = withDecided([large], undefined);

        expect(withDecided([large, large], seen)).toBe(seen);
        expect(withDecided([large, large], undefined)).toEqual(new Set([faceKey(large)]));
    });
});

describe("whether a detection leaves any face to restore", () => {
    it("finds one where a face at or below the tile sits among larger ones", () => {
        expect(noFaceToRestore([large, square(2000, 300), square(3000, 900)])).toBe(false);
    });

    it("finds none where every face is larger than the tile", () => {
        expect(noFaceToRestore([large, square(3000, 900)])).toBe(true);
    });

    it("finds one in a face exactly at the tile", () => {
        expect(noFaceToRestore([small])).toBe(false);
    });

    it("finds none in a detection that found nobody", () => {
        expect(noFaceToRestore([])).toBe(true);
    });
});

describe("withFaces", () => {
    it("puts the faces into every face recovery and leaves the rest as they are", () => {
        const upscale: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 };
        const recovery = { family: "face_recovery", codename: "athens", precision: "fp32" } as Operation;
        const faces = [at(10)];

        expect(withFaces([recovery, upscale], faces)).toEqual([{ ...recovery, faces }, upscale]);
    });
});
