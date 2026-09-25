import { describe, expect, it } from "vitest";
import { constrain, constrainPosition, fitInside, imageFraction } from "./geometry";

/** A landscape pane, which is the shape the canvas is in every window this application opens in. */
const PANE = { width: 1000, height: 600 };

describe("fitInside", () => {
    it("draws a landscape photograph across a pane of the other shape", () => {
        // 3000x2000 in a 1000x600 pane: the height is what runs out first, so the photograph is
        // 900x600 and there is margin either side of it rather than above and below.
        expect(fitInside(PANE, { width: 3000, height: 2000 })).toEqual({ width: 900, height: 600 });
    });

    it("draws a portrait photograph across a pane of the other shape", () => {
        // 2000x3000 in the same pane: now the height runs out far sooner, and the margin moves.
        expect(fitInside(PANE, { width: 2000, height: 3000 })).toEqual({ width: 400, height: 600 });
    });

    it("enlarges a photograph smaller than its pane rather than leaving it at its own size", () => {
        // The case `object-contain` under a `max-*` never covered: a 200x100 photograph was drawn
        // 200px wide in a 1000px canvas. 1x has to mean fitted for every photograph, or the slider
        // is a magnification against a size the user cannot see.
        expect(fitInside(PANE, { width: 200, height: 100 })).toEqual({ width: 1000, height: 500 });
    });

    it("is zero while there is nothing measured to fit into", () => {
        // A pane mounted hidden, a pane measured before layout, and every pane in jsdom. The pane
        // draws the `<img>` with no explicit size rather than a photograph of zero width.
        expect(fitInside({ width: 0, height: 0 }, { width: 3000, height: 2000 })).toEqual({ width: 0, height: 0 });
        expect(fitInside(PANE, { width: 0, height: 0 })).toEqual({ width: 0, height: 0 });
    });
});

describe("constrain", () => {
    it("centres a photograph no larger than its pane, wherever it is dragged to", () => {
        // Nothing to move at 1x, and "fitted" means centred - so every position on this axis is the
        // same position, which is what makes a drag of a fitted image a no-op rather than a jump.
        expect(constrain(0, 900, 1000)).toBe(50);
        expect(constrain(-400, 900, 1000)).toBe(50);
        expect(constrain(700, 900, 1000)).toBe(50);
        expect(constrain(0, 1000, 1000)).toBe(0);
    });

    it("holds a magnified photograph at its own left edge", () => {
        // Dragged right past its own start: the left edge stops at the pane's, rather than letting
        // empty space open beside a photograph that has more of itself to show.
        expect(constrain(120, 1800, 1000)).toBe(0);
    });

    it("holds a magnified photograph at its own right edge", () => {
        // 1800px of photograph in a 1000px pane leaves 800px of travel, so -800 is the far edge.
        expect(constrain(-2000, 1800, 1000)).toBe(-800);
        expect(constrain(-800, 1800, 1000)).toBe(-800);
    });

    it("leaves a position already between the edges alone", () => {
        expect(constrain(-300, 1800, 1000)).toBe(-300);
    });
});

describe("constrainPosition", () => {
    it("clamps one axis and centres the other at the same scale", () => {
        // A photograph magnified past the pane's width but not its height - the case that makes the
        // two axes worth keeping independent.
        expect(constrainPosition({ x: -2000, y: 300 }, { width: 1800, height: 400 }, PANE)).toEqual({
            x: -800,
            y: 100,
        });
    });
});

describe("imageFraction", () => {
    it("reports where in the photograph a point is", () => {
        expect(imageFraction(450, 1800)).toBe(0.25);
        expect(imageFraction(0, 1800)).toBe(0);
        expect(imageFraction(1800, 1800)).toBe(1);
    });

    it("resolves a point in the margin beside the photograph to its nearest edge", () => {
        expect(imageFraction(-200, 1800)).toBe(0);
        expect(imageFraction(2400, 1800)).toBe(1);
    });

    it("is the middle while there is nothing measured to take a fraction of", () => {
        expect(imageFraction(300, 0)).toBe(0.5);
    });
});
