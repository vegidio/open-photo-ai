import { clampTo } from "@/lib/utils";

/**
 * Where the photograph is drawn, and nothing about how it is drawn.
 *
 * Four pure functions and one coordinate system, which is what keeps the pane, the bounds a pan
 * stops at, the point a wheel zoom is anchored on and the rectangle the sidebar draws from coming to
 * disagree - the reference's own comment records paying for that once, when a measurement taken at
 * mount left "the wheel anchor, `constrainPosition`, and the viewport rect" all working off stale
 * numbers together.
 *
 * **The coordinate system.** The `<img>` is drawn at its fitted size with its top-left at the pane's
 * top-left, and moved by `translate(x, y) scale(s)` about a `top left` origin. So a position is the
 * offset of the photograph's top-left corner from the pane's, in pane pixels, and the photograph
 * occupies `position .. position + fitted * scale`. There is no letterboxing to add back: the pane
 * does not use `object-contain`, precisely because the browser's centring is an offset nothing here
 * could read.
 */

/** A size in pane pixels. Zero on either axis until the pane has been measured. */
export type Size = { width: number; height: number };

/** The offset of the photograph's top-left corner from the pane's, in pane pixels. */
export type Position = { x: number; y: number };

/**
 * The size the photograph is drawn at when it is drawn whole - which is what 1x means.
 *
 * **Not clamped to the photograph's own size**, so a photograph smaller than the pane is enlarged to
 * fill it. That is the design's own plate arithmetic (`Math.min(availW / f.w, PANE_H / f.h)`, no
 * clamp) and the reference's measurement callback, and it is what makes the slider mean something:
 * 1x is "fitted" for every photograph rather than a magnification whose relationship to the pane
 * depends on a number the user cannot see.
 *
 * Named `fitInside` rather than the design's `fit`, which is Jasmine's focused-test alias: Biome's
 * `noFocusedTests` reads every call of it as a test left focused, and five suppressions for one false
 * positive is a worse trade than a longer name.
 *
 * Zero for a pane that has not been measured, or an image of no size - which is what a pane mounted
 * hidden, and every pane in jsdom, reports. The pane draws the `<img>` with no explicit size until
 * it has a real measurement rather than drawing a photograph of zero width.
 */
export const fitInside = (container: Size, image: Size): Size => {
    if (container.width <= 0 || container.height <= 0 || image.width <= 0 || image.height <= 0) {
        return { width: 0, height: 0 };
    }

    const scale = Math.min(container.width / image.width, container.height / image.height);

    return { width: image.width * scale, height: image.height * scale };
};

/**
 * One axis of the rule that a photograph is never dragged away from its pane.
 *
 * Centred while it is no larger than the pane - there is nothing to move, and the centring is the
 * whole of what "fitted" means - and held between its own edges once it is larger, so no edge of it
 * ever comes inside the pane.
 *
 * The reference's `constrainPosition`, kept per-axis rather than taking two sizes, because the two
 * axes are genuinely independent: a portrait photograph in a landscape pane is clamped on one and
 * centred on the other at the same scale.
 */
export const constrain = (position: number, scaledSize: number, containerSize: number): number => {
    if (scaledSize <= containerSize) return (containerSize - scaledSize) / 2;

    return Math.max(containerSize - scaledSize, Math.min(0, position));
};

/** Both axes of {@link constrain}, which is how every caller but a test uses it. */
export const constrainPosition = (position: Position, scaled: Size, container: Size): Position => ({
    x: constrain(position.x, scaled.width, container.width),
    y: constrain(position.y, scaled.height, container.height),
});

/**
 * A point of the displayed photograph as a fraction in [0, 1] of its drawn size.
 *
 * Fractions rather than pixels because the sibling pane has to apply the same anchor to its own
 * container: in side by side the two panes are the same size today, so pixels would work - and they
 * stop working the moment the panes differ, which is one layout change away. The reference chose
 * fractions for exactly this reason.
 *
 * Clamped, so a pointer in the margin beside the photograph resolves to its nearest edge; the middle
 * before there is anything measured to take a fraction of.
 */
export const imageFraction = (offset: number, scaledSize: number): number =>
    scaledSize > 0 ? clampTo(offset / scaledSize, 0, 1) : 0.5;
