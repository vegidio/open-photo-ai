import { describe, expect, it } from "vitest";
import { FREE_RATIO, RATIOS, ratioByKey } from "./ratios";

describe("the aspect-ratio table", () => {
    it("offers the ten the design draws", () => {
        expect(RATIOS.map((option) => option.key)).toEqual([
            "free",
            "square",
            "5:4",
            "4:5",
            "4:3",
            "3:4",
            "3:2",
            "2:3",
            "16:9",
            "9:16",
        ]);
    });

    it("carries every option's inverse, and it is in the table", () => {
        for (const option of RATIOS) {
            expect(ratioByKey(option.inverse), `${option.key} has no inverse in the table`).toBeDefined();
        }
    });

    it("pairs each option with the one whose ratio is its reciprocal", () => {
        for (const option of RATIOS) {
            const inverse = ratioByKey(option.inverse);

            if (option.value === undefined) {
                // Free is its own inverse: there is no constraint to transpose, and swapping under it
                // transposes the box and leaves the grid alone.
                expect(inverse?.value).toBeUndefined();
                continue;
            }

            // `toBeCloseTo` rather than an equality: `16 / 9` times `9 / 16` is not 1 in binary
            // floating point, which is why the table declares its pairs (see `RatioOption.inverse`).
            expect(inverse?.value).toBeCloseTo(1 / option.value, 12);
        }
    });

    it("swaps back to where it started", () => {
        for (const option of RATIOS) {
            expect(ratioByKey(option.inverse)?.inverse).toBe(option.key);
        }
    });

    it("takes its label from the catalogue for the two that are words and no others", () => {
        const translated = RATIOS.filter((option) => option.translate).map((option) => option.key);

        expect(translated).toEqual(["free", "square"]);
    });

    it("draws each option in its own proportions", () => {
        for (const option of RATIOS) {
            if (option.value === undefined) continue;

            // Within a pixel of the ratio it names: the design sizes the boxes by eye so that 5:4 and
            // 4:3 are told apart in a 45px well, which is a touch further apart than the numbers are.
            expect(option.box.width / option.box.height).toBeCloseTo(option.value, 0.6);
        }
    });

    it("starts on the option with no constraint", () => {
        expect(ratioByKey(FREE_RATIO)?.value).toBeUndefined();
    });
});
