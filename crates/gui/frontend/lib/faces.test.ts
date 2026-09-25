import { describe, expect, it } from "vitest";
import type { FaceChoice, Operation } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import { choiceAfter, enabledFaces, faceKey, isKept, withChoice } from "./faces";

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
    restorable: true,
    key: `${left},${top},${left + 3},${top + 3}`,
});

/** A face too large to restore, which the default leaves alone. */
const large = (left: number): Face => ({ ...at(left), restorable: false });

const choice = (skipped: Face[] = [], restored: Face[] = []): FaceChoice => ({
    skipped: skipped.map(faceKey),
    restored: restored.map(faceKey),
});

describe("a face's key", () => {
    it("is the key Rust published on it, not one composed here", () => {
        const published: Face = { ...at(10), key: "whatever Rust wrote" };

        expect(faceKey(published)).toBe("whatever Rust wrote");
    });
});

describe("whether a run keeps a face", () => {
    it("follows the face's own default where nobody chose", () => {
        expect(isKept(at(0), undefined)).toBe(true);
        expect(isKept(large(0), undefined)).toBe(false);
        expect(isKept(at(0), choice())).toBe(true);
    });

    it("follows the user where they turned a restorable face off or a large one on", () => {
        expect(isKept(at(0), choice([at(0)]))).toBe(false);
        expect(isKept(large(0), choice([], [large(0)]))).toBe(true);
    });

    it("ignores a choice made about a face at another framing", () => {
        expect(isKept(at(0), choice([at(900)]))).toBe(true);
    });
});

describe("the faces a run restores", () => {
    it("preserves the order of the faces that survive", () => {
        // Rust folds the order into the run cache tag, so a reordered selection would be a different result.
        const found = [at(40), large(0), at(10), at(20)];

        expect(enabledFaces(found, choice([at(20)]))).toEqual([at(40), at(10)]);
    });

    it("answers an empty list for a photograph with nobody in it", () => {
        expect(enabledFaces([], choice([at(10)]))).toEqual([]);
    });
});

describe("the choice the picker commits", () => {
    const shown = [at(0), at(20), large(40)];
    const defaults = new Set([faceKey(large(40))]);

    it("hands back the choice it was given where the faces are left as they were", () => {
        const held = choice([at(0)]);

        expect(choiceAfter(shown, new Set([faceKey(at(0)), faceKey(large(40))]), held)).toBe(held);
        expect(choiceAfter(shown, defaults, undefined)).toBeUndefined();
    });

    it("records only the exceptions to each face's default", () => {
        // A restorable face turned off, and a large one turned on: nothing else is written.
        const off = new Set([faceKey(at(20))]);

        expect(choiceAfter(shown, off, undefined)).toEqual(choice([at(20)], [large(40)]));
    });

    it("drops an exception once the face is back at its default", () => {
        expect(choiceAfter(shown, defaults, choice([at(0)], [large(40)]))).toEqual(choice());
    });

    it("keeps the exceptions made at another framing", () => {
        expect(choiceAfter(shown, new Set([...defaults, faceKey(at(0))]), choice([at(900)]))).toEqual(
            choice([at(900), at(0)]),
        );
    });
});

describe("withChoice", () => {
    const upscale: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", parameters: { scale: 2 } };
    const recovery: Operation = { family: "face_recovery", codename: "athens", precision: "fp32", parameters: {} };

    it("puts the choice into every face recovery and leaves the rest as they are", () => {
        const held = choice([at(0)]);

        expect(withChoice([recovery, upscale], held)).toEqual([{ ...recovery, faces: held }, upscale]);
    });

    it("sends no choice where nobody made one, so every face follows its default", () => {
        expect(withChoice([recovery], undefined)).toEqual([recovery]);
    });

    it("answers a copy even for a stack with no face recovery", () => {
        const stack = [upscale];

        expect(withChoice(stack, choice())).not.toBe(stack);
        expect(withChoice(stack, choice())).toEqual(stack);
    });
});
