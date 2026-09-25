import { act, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EnhancementRun } from "@/hooks/useEnhancementRun";
import "@/i18n";
import type { Family } from "@/ipc/catalogue";
import { cropQuery } from "@/ipc/crop";
import type { RunProgress } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import { DRAWER_HEIGHT, ZOOM_MAX, ZOOM_MIN, ZOOM_WHEEL_STEP } from "@/lib/constants";
import { faceKey } from "@/lib/faces";
import { useDrawerStore } from "@/stores/drawer";
import { useFacesStore } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { usePreviewStore } from "@/stores/preview";
import { type ImageTransform, useTransformStore } from "@/stores/transform";
import { FRAMING, frame, HOLIDAY, openFiles, render, resetCropStore, resetFileStore, SUNSET } from "@/test/support";
import { PreviewImage } from "./PreviewImage";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of. Mocked to
// answer the identity it was asked for, so a test can tell which photograph a pane is drawing without
// restating the platform-dependent URL - which is the one thing `ipc/images.ts` exists to own.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// The run itself is `hooks/useEnhancementRun.test.tsx`'s subject - what it asks for, what it stops
// and what it does with the answers. Mocked here so this file stays about what the canvas *draws*
// given a run, rather than about driving one through the seam.
vi.mock("@/hooks/useEnhancementRun", () => ({
    useEnhancementRun: vi.fn((): EnhancementRun => ({ running: false, fraction: 0, ...run })),
}));

/**
 * What the mocked hook answers, which a case sets before it draws.
 *
 * Partial, so that the position on the bar is stated only by the cases the position is about. It is
 * the hook's own figure and not the report's - a face recovery is two runs and one bar, and which
 * of the two the canvas draws is what the three cases below pin.
 */
let run: Partial<EnhancementRun> = { running: false };

/** The result of a run, as the hook reports one. A 2x enlargement of HOLIDAY's 3000x2000. */
const ENHANCED = { identity: "fedcba9876543210", width: 6000, height: 4000 };

/** One report about a run in flight. */
const progress = (extra: Partial<RunProgress> = {}): RunProgress => ({
    run: "run-1",
    operation: "Kyoto 2x (FP32)",
    family: "upscale",
    stage: "running",
    chainFraction: 0.62,
    ...extra,
});

const bar = () => document.querySelector("[data-slot='preview-progress']");

/**
 * How far the bar says the run has got.
 *
 * Read off the indicator's own transform, because that is how the shared `Progress` draws a value -
 * the slide of that transform is the animation this bar is built on. Its `progressbar` role carries
 * no `aria-valuenow`: the vendored component keeps `value` for the transform and does not forward it
 * to the Radix root, so every bar in this application reads as indeterminate to a screen reader.
 */
const filled = () => bar()?.querySelector("[data-slot='progress-indicator']");

/**
 * A pane of a known size, which jsdom otherwise answers 0 for.
 *
 * jsdom lays nothing out, so every `getBoundingClientRect` is a zero rect and every pane measures
 * itself as nothing - which is exactly the state `fitInside` returns zero for, and every assertion about
 * where a photograph is drawn would be an assertion about zero. One rect for every element is enough
 * here: the two panes are the same size in every mode, and the canvas the divider measures itself
 * against is the same box again.
 *
 * Restored by the `restoreMocks` in vite.config.ts, so nothing has to undo it.
 */
const PANE = { width: 1000, height: 600 };

const stubPaneGeometry = ({ width, height } = PANE) =>
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
        x: 0,
        y: 0,
        top: 0,
        left: 0,
        right: width,
        bottom: height,
        width,
        height,
        toJSON: () => ({}),
    });

/** HOLIDAY is 3000x2000, so in a 1000x600 pane the height runs out first and it is drawn 900x600. */
const FITTED = { width: 900, height: 600 };

const panes = () => screen.getAllByRole("img", { name: "Preview" });

/** What a pane has actually drawn, read back off the one style that says so. */
const drawn = (index = 0) => {
    const style = panes()[index]?.style.transform ?? "";
    const [, x, y, scale] = /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(style) ?? [];

    return { x: Number(x), y: Number(y), scale: Number(scale) };
};

/** Magnifies the current photograph by whichever route a test is not itself about. */
const magnify = (transform: ImageTransform) =>
    act(() => useTransformStore.getState().setTransform(HOLIDAY.identity ?? "", transform));
const sources = () => panes().map((pane) => pane.getAttribute("src"));
const divider = () => document.querySelector("[data-slot='preview-divider']");

const draw = (mode: "full" | "side" | "split") => {
    usePreviewStore.setState({ previewMode: mode });

    return render(<PreviewImage />);
};

beforeEach(() => {
    run = { running: false };
    resetFileStore();
    resetCropStore();
    useDrawerStore.setState(useDrawerStore.getInitialState(), true);
    useTransformStore.setState(useTransformStore.getInitialState(), true);
    usePreviewStore.setState({ previewMode: "side" });
    openFiles(HOLIDAY);
});

describe("the canvas", () => {
    it("draws the two panes side by side, each labelled", () => {
        // With a result landed, which is the state in which the two chips differ - the labelling
        // rule itself is `the pane labels` below.
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        expect(panes()).toHaveLength(2);
        expect(screen.getByText("Original")).toBeInTheDocument();
        expect(screen.getByText("Enhanced")).toBeInTheDocument();
        expect(divider()).toBeNull();
    });

    it("draws the enhanced image alone across the whole canvas", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("full");

        // One pane, and it is the enhanced one: full is the mode that shows only the result.
        expect(panes()).toHaveLength(1);
        expect(screen.queryByText("Original")).not.toBeInTheDocument();
        expect(screen.getByText("Enhanced")).toBeInTheDocument();
    });

    it("draws the two superimposed, divided, when split is chosen", () => {
        run = { running: false, enhanced: ENHANCED };

        const { container } = draw("split");

        expect(panes()).toHaveLength(2);
        expect(divider()).toBeInTheDocument();

        // The enhanced pane is out of the flex flow and clipped from the divider rightward, which is
        // what makes it the same photograph superimposed rather than a second half-width copy. The
        // clip is a style rather than a class now that the divider moves: it is one number with the
        // divider's own `left`, where the two used to be a matched pair of literals.
        const enhanced = container.querySelector("[style*='clip-path']");
        expect(enhanced).toBeInTheDocument();
        expect(enhanced).toContainElement(screen.getByText("Enhanced"));
    });

    it("asks for one and the same photograph in every pane of every mode", () => {
        // The identity and the bound are the same, so the second pane and every mode change is a
        // cache hit on bytes the webview already has rather than a second decode.
        const wanted = `opai://localhost/${HOLIDAY.identity}`;

        for (const mode of ["full", "side", "split"] as const) {
            const { unmount } = draw(mode);

            expect(sources().every((source) => source === wanted)).toBe(true);
            unmount();
        }
    });

    it("asks for the photograph at its own size, with no bound", () => {
        draw("full");

        // `renditionUrl(identity)` rather than a canvas-sized rendition: the zoom a later slice adds
        // goes to 8x, and a bounded rendition would be an eight-times-magnified thumbnail.
        expect(sources()[0]).not.toContain("size=");
    });

    it("asks for the framing when the current image carries one", () => {
        frame(HOLIDAY);

        draw("side");

        // The same framing the sidebar's miniature asks for, at the canvas's own (unbounded) size.
        // Both regions read one store keyed by identity, so there is no second place a framing can be
        // spelled differently.
        const wanted = `opai://localhost/${HOLIDAY.identity}?crop=${cropQuery(FRAMING)}`;

        expect(sources().every((source) => source === wanted)).toBe(true);
    });

    it("asks for no framing at all while nothing is framed", () => {
        draw("side");

        // Which is every photograph in this application until the Crop/Rotate dialog ships: the URL
        // is byte-for-byte the one the canvas has always asked for.
        expect(sources().every((source) => source === `opai://localhost/${HOLIDAY.identity}`)).toBe(true);
    });

    it("does not frame a result that was already made from the framing", () => {
        frame(HOLIDAY);
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        // The run was handed the framed pixels, so what it produced is already the enhancement of the
        // framing - asking for it to be framed again would cut the rectangle out of it a second time.
        const [original, enhanced] = sources();

        expect(original).toContain(`crop=${cropQuery(FRAMING)}`);
        expect(enhanced).toBe(`opai://localhost/${ENHANCED.identity}`);
    });

    it("draws nothing rather than the wrong pixels for a file it could not read", () => {
        resetFileStore();
        // A file whose bytes could not be read carries no identity, so nothing addresses it and the
        // protocol can never be asked for it. Built by leaving the key out rather than by setting it
        // to `undefined`, which under `exactOptionalPropertyTypes` is not the same record.
        const { identity, ...unreadable } = HOLIDAY;
        openFiles(unreadable);

        draw("side");

        expect(sources()).toEqual([null, null]);
    });
});

/**
 * Where the pane labels sit, which is the one thing on the canvas that has to know the drawer moved.
 *
 * The drawer slides over the canvas rather than resizing it, so a chip that stayed 10px above the
 * canvas's own bottom edge would be behind the strip for as long as it was unfolded - which is what
 * the design's `chipBottom` moves, and nothing else on this side does.
 */
describe("the pane labels", () => {
    const chip = (label: string) => screen.getByText(label);

    // A landed result throughout, so the two chips carry different words and `getByText` can name
    // either of them. What each one says is `what the pane labels say` below.
    beforeEach(() => {
        run = { running: false, enhanced: ENHANCED };
    });

    it("ride above the canvas's bottom edge while the drawer is folded", () => {
        draw("side");

        expect(chip("Original")).toHaveStyle({ bottom: "10px" });
        expect(chip("Enhanced")).toHaveStyle({ bottom: "10px" });
    });

    it("ride above the strip once the drawer is unfolded", () => {
        useDrawerStore.setState({ open: true });
        draw("side");

        // Derived from the drawer's own constant rather than written as 138, so the rule is what the
        // test states instead of today's arithmetic.
        expect(chip("Original")).toHaveStyle({ bottom: `${DRAWER_HEIGHT + 10}px` });
        expect(chip("Enhanced")).toHaveStyle({ bottom: `${DRAWER_HEIGHT + 10}px` });
    });

    it("lift in every comparison, not only the one with two panes", () => {
        useDrawerStore.setState({ open: true });
        draw("full");

        expect(chip("Enhanced")).toHaveStyle({ bottom: `${DRAWER_HEIGHT + 10}px` });
    });
});

/**
 * Where the photograph is drawn, which is the whole of what replaced `object-contain`.
 *
 * The browser's own centring was an offset nothing here could read back, and the bounds a pan stops
 * at, the point a wheel zoom is anchored on and the rectangle the sidebar draws are each derived
 * from knowing exactly where the photograph's top-left corner is.
 */
describe("the photograph's own box", () => {
    it("is drawn at the size that fits, whole and centred, before anything magnifies it", () => {
        stubPaneGeometry();
        draw("full");

        // 900x600 in a 1000x600 pane, so the margin is 50px either side and none above or below.
        expect(panes()[0]).toHaveStyle({ width: "900px", height: "600px" });
        expect(drawn()).toEqual({ x: 50, y: 0, scale: 1 });
    });

    it("is drawn at the framing's shape rather than the file's", () => {
        stubPaneGeometry();
        // A portrait framing of a landscape photograph: 1200x1600 out of HOLIDAY's 3000x2000, so the
        // height runs out first and 600px of pane holds 450px of width.
        frame(HOLIDAY);

        draw("full");

        // Laid out from the crop's own rectangle, which is what makes the box right *before* the
        // pixels arrive - the whole point of `framedDimensions`. Read off the file it would be
        // 900x600, the wrong shape entirely, for the length of a decode and then permanently.
        expect(panes()[0]).toHaveStyle({ width: "450px", height: "600px" });
        expect(drawn()).toEqual({ x: 275, y: 0, scale: 1 });
    });

    it("is drawn at the file's own shape while nothing is framed", () => {
        stubPaneGeometry();
        draw("full");

        // Unchanged, which is this slice's acceptance criterion everywhere it appears.
        expect(panes()[0]).toHaveStyle({ width: `${FITTED.width}px`, height: `${FITTED.height}px` });
    });

    it("draws a magnified photograph at the position it was left at", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });

        draw("full");

        // The element keeps its fitted size and the scale is a transform, so the compositor holds
        // one decoded bitmap and a matrix rather than a photograph resized to eight times a wall.
        expect(panes()[0]).toHaveStyle({ width: "900px", height: "600px" });
        expect(drawn()).toEqual({ x: -100, y: -50, scale: 2 });
    });

    it("holds a magnified photograph inside the pane it is in", () => {
        stubPaneGeometry();
        // 1800x1200 of photograph in a 1000x600 pane leaves 800px and 600px of travel; a position
        // past either is a photograph dragged away from its own edge, which never happens.
        magnify({ scale: 2, x: 400, y: -5000 });

        draw("full");

        expect(drawn()).toEqual({ x: 0, y: -600, scale: 2 });
    });

    it("draws nothing of a size it has not measured", () => {
        // A pane mounted hidden, a pane measured before layout - and every pane in jsdom without the
        // stub above. The `<img>` carries no width rather than a photograph of zero width.
        draw("full");

        expect(panes()[0]?.style.width).toBe("");
    });

    it("draws both panes from the one transform", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });

        draw("side");

        // Two panes showing different parts of a photograph are not a comparison, which is the whole
        // of what the second one is for.
        expect(drawn(0)).toEqual(drawn(1));
        expect(drawn(0)).toEqual({ x: -100, y: -50, scale: 2 });
    });

    it("draws the same magnification in whichever comparison is chosen", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });

        // The transform is a property of the photograph rather than of the layout drawing it, so
        // changing the comparison is a change of how many panes there are and of nothing else.
        for (const mode of ["full", "side", "split"] as const) {
            const { unmount } = draw(mode);

            expect(drawn()).toEqual({ x: -100, y: -50, scale: 2 });
            unmount();
        }
    });
});

/** Dragging a pane, which is the pan - and the one gesture that is coalesced rather than immediate. */
describe("panning", () => {
    let frames: FrameRequestCallback[] = [];

    beforeEach(() => {
        frames = [];
        // Held rather than run, so a test can say how many writes a drag made *before* the frame it
        // was coalesced into - which is the whole of what the coalescing is.
        vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((callback) => {
            frames.push(callback);

            return frames.length;
        });
        vi.spyOn(globalThis, "cancelAnimationFrame").mockImplementation(() => {});
    });

    afterEach(() => {
        frames = [];
    });

    const runFrame = () =>
        act(() => {
            for (const callback of frames) callback(0);
        });

    const pane = () => document.querySelector("[data-slot='preview-pane']") as HTMLElement;

    const dragBy = (element: HTMLElement, steps: { x: number; y: number }[]) => {
        fireEvent.pointerDown(element, { pointerId: 1, clientX: 500, clientY: 300 });
        for (const step of steps) {
            fireEvent.pointerMove(element, { pointerId: 1, clientX: 500 + step.x, clientY: 300 + step.y });
        }
    };

    it("writes once per frame however many pointer events a drag delivers", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        draw("full");

        dragBy(pane(), [
            { x: -10, y: -10 },
            { x: -20, y: -20 },
            { x: -30, y: -30 },
        ]);

        // Three moves, one frame asked for, and nothing in the store until it runs: without this
        // every pointer frame re-rendered both panes and republished the viewport.
        expect(frames).toHaveLength(1);
        expect(useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "")).toEqual({
            scale: 2,
            x: -100,
            y: -50,
        });

        runFrame();

        // The last of the three, not the first: the pending write is replaced, not queued.
        expect(drawn()).toEqual({ x: -130, y: -80, scale: 2 });
    });

    it("stops a magnified photograph at its own edge", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        draw("full");

        dragBy(pane(), [{ x: 4000, y: 4000 }]);
        runFrame();

        expect(drawn()).toEqual({ x: 0, y: 0, scale: 2 });
    });

    it("leaves a fitted photograph whole and centred however it is dragged", () => {
        stubPaneGeometry();
        draw("full");

        dragBy(pane(), [{ x: -400, y: -200 }]);
        runFrame();

        // There is nothing to move at 1x, and "fitted" means centred - so every position on this
        // axis resolves to the same one.
        expect(drawn()).toEqual({ x: 50, y: 0, scale: 1 });
    });

    it("takes the pointer capture, so a drag that leaves the pane keeps following it", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        draw("full");

        const element = pane();
        fireEvent.pointerDown(element, { pointerId: 1, clientX: 500, clientY: 300 });
        expect(element.hasPointerCapture(1)).toBe(true);

        fireEvent.pointerUp(element, { pointerId: 1 });
        expect(element.hasPointerCapture(1)).toBe(false);
    });

    it("keeps the last position of a drag that ends with the pane going away", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        const { unmount } = draw("full");

        dragBy(pane(), [{ x: -30, y: -30 }]);
        unmount();

        // Flushed rather than cancelled: the coalesced write is the only one that ever reaches the
        // store, so dropping it loses up to a frame of panning.
        expect(useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "")).toEqual({
            scale: 2,
            x: -130,
            y: -80,
        });
    });
});

/** The wheel, and the trackpad pinch the webview delivers as one. */
describe("the wheel", () => {
    const wheel = (element: Element, { deltaY = -100, clientX = 500, clientY = 300 } = {}) => {
        const event = new WheelEvent("wheel", { deltaY, clientX, clientY, cancelable: true, bubbles: true });
        act(() => {
            element.dispatchEvent(event);
        });

        return event;
    };

    const pane = () => document.querySelector("[data-slot='preview-pane']") as HTMLElement;

    it("does not let the gesture scroll whatever is under it", () => {
        stubPaneGeometry();
        draw("full");

        // React's synthetic `onWheel` is registered passive, so `preventDefault` on it is a silent
        // no-op - which is why the listener is a native, non-passive one.
        expect(wheel(pane()).defaultPrevented).toBe(true);
    });

    it("magnifies by one step, and reduces by one", () => {
        stubPaneGeometry();
        draw("full");

        wheel(pane());
        expect(drawn().scale).toBeCloseTo(ZOOM_MIN + ZOOM_WHEEL_STEP, 10);

        wheel(pane(), { deltaY: 100 });
        expect(drawn().scale).toBeCloseTo(ZOOM_MIN, 10);
    });

    it("keeps the point under the pointer under the pointer", () => {
        stubPaneGeometry();
        draw("full");

        // The pointer is over the middle of the photograph: 500px across a pane whose photograph
        // starts at 50 and is 900 wide, so the anchor is the halfway point of it on both axes.
        const before = drawn();
        const anchor = {
            x: (500 - before.x) / (FITTED.width * before.scale),
            y: (300 - before.y) / (FITTED.height * before.scale),
        };

        wheel(pane());

        const after = drawn();
        expect(after.x + anchor.x * FITTED.width * after.scale).toBeCloseTo(500, 6);
        expect(after.y + anchor.y * FITTED.height * after.scale).toBeCloseTo(300, 6);
        expect(after.scale).toBeGreaterThan(before.scale);
    });

    it("holds at each end of the range rather than passing it", () => {
        stubPaneGeometry();
        magnify({ scale: ZOOM_MAX, x: 0, y: 0 });
        draw("full");

        wheel(pane());
        expect(drawn().scale).toBe(ZOOM_MAX);

        magnify({ scale: ZOOM_MIN, x: 0, y: 0 });
        wheel(pane(), { deltaY: 100 });
        expect(drawn().scale).toBe(ZOOM_MIN);
    });

    it("records where the anchoring landed, not where the photograph was standing", () => {
        stubPaneGeometry();
        draw("full");

        // Anchored at the far right of the photograph, which is where the correction is largest.
        for (let notch = 0; notch < 6; notch += 1) wheel(pane(), { clientX: 950 });

        // The store is what a photograph is handed back from when it becomes current again, so a
        // store holding the pre-zoom position would return the user to a crop one notch's anchoring
        // correction away from the one on screen - `anchor * fitted * ZOOM_WHEEL_STEP`, which is 43px
        // of this 1000px pane.
        const stored = useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "");
        expect(stored?.x).toBeCloseTo(drawn().x, 6);
        expect(stored?.y).toBeCloseTo(drawn().y, 6);
    });

    it("returns a photograph to the part of it that was being examined", () => {
        stubPaneGeometry();
        openFiles(SUNSET);
        act(() => useFileStore.getState().setCurrentIndex(0));
        draw("full");

        for (let notch = 0; notch < 6; notch += 1) wheel(pane(), { clientX: 950 });
        const examined = drawn();

        act(() => useFileStore.getState().setCurrentIndex(1));
        act(() => useFileStore.getState().setCurrentIndex(0));

        expect(drawn()).toEqual(examined);
    });

    it("leaves the photograph alone for a gesture that is not over a pane", () => {
        stubPaneGeometry();
        draw("full");

        const before = drawn();
        // The listener is the pane's own rather than the document's, which is what keeps a wheel over
        // the sidebar or the drawer from magnifying the canvas behind them.
        wheel(document.body);

        expect(drawn()).toEqual(before);
    });
});

/** What the canvas reports to the sidebar, which is the one thing the two regions share. */
describe("the published viewport", () => {
    const viewport = () => useTransformStore.getState().viewport;

    it("is the whole photograph while it is drawn whole", () => {
        stubPaneGeometry();
        draw("full");

        // The true answer rather than a special case: all of the photograph is being shown.
        expect(viewport()).toEqual({ x: 0, y: 0, width: 1, height: 1 });
    });

    it("is the part of a magnified photograph that is inside the pane", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: 0, y: 0 });

        draw("full");

        // 1800x1200 of photograph against a 1000x600 pane, held at its top-left corner.
        expect(viewport()).toEqual({ x: 0, y: 0, width: 1000 / 1800, height: 0.5 });
    });

    it("is written by the enhanced pane alone", () => {
        stubPaneGeometry();
        const setViewport = vi.fn();
        useTransformStore.setState({ setViewport });

        // Counted against the mode that draws the enhanced pane by itself, rather than against a
        // literal: what this is about is that the second pane adds no writer, not how many times one
        // pane resolves itself while it is being measured.
        const alone = draw("full");
        const once = setViewport.mock.calls.length;
        alone.unmount();

        setViewport.mockClear();
        draw("side");

        // Both panes are always the same size, so a second writer would be producing a value
        // identical to the first's on every frame of every pan - which is the re-render storm the
        // reference added a comparison to undo.
        expect(setViewport).toHaveBeenCalledTimes(once);
    });
});

/** The split comparison's divider, which is the only draggable thing on a canvas that also pans. */
describe("the split divider", () => {
    const divider = () => screen.getByRole("slider", { name: "Divider" });
    const clipped = () => document.querySelector("[style*='clip-path']") as HTMLElement;

    it("starts at the middle of the canvas", () => {
        stubPaneGeometry();
        draw("split");

        expect(divider()).toHaveStyle({ left: "50%" });
        expect(clipped().style.clipPath).toBe("inset(0 0 0 50%)");
        expect(divider()).toHaveAttribute("aria-valuenow", "50");
    });

    it("moves the divider and the clip together", () => {
        stubPaneGeometry();
        draw("split");

        fireEvent.pointerDown(divider(), { pointerId: 1, clientX: 250, clientY: 300 });
        fireEvent.pointerMove(divider(), { pointerId: 1, clientX: 250, clientY: 300 });

        // One number now, where the divider's `left` and the enhanced pane's clip were a matched
        // pair of literals for as long as the divider could not move.
        expect(divider()).toHaveStyle({ left: "25%" });
        expect(clipped().style.clipPath).toBe("inset(0 0 0 25%)");
    });

    it("does not move when a pane is dragged", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        draw("split");

        const pane = document.querySelector("[data-slot='preview-pane']") as HTMLElement;
        fireEvent.pointerDown(pane, { pointerId: 1, clientX: 500, clientY: 300 });
        fireEvent.pointerMove(pane, { pointerId: 1, clientX: 200, clientY: 300 });

        // The panes are the pan surface, so a drag started on one and a drag started on the divider
        // have to mean different things.
        expect(divider()).toHaveStyle({ left: "50%" });
    });

    it("moves under the arrow keys, so the third comparison is operable without a mouse", () => {
        stubPaneGeometry();
        draw("split");

        fireEvent.keyDown(divider(), { key: "ArrowLeft" });
        expect(divider()).toHaveAttribute("aria-valuenow", "48");

        fireEvent.keyDown(divider(), { key: "ArrowRight" });
        fireEvent.keyDown(divider(), { key: "ArrowRight" });
        expect(divider()).toHaveAttribute("aria-valuenow", "52");
    });

    it("stays where the user put it when another image becomes the current one", () => {
        stubPaneGeometry();
        openFiles(SUNSET);
        act(() => useFileStore.getState().setCurrentIndex(0));
        draw("split");

        fireEvent.pointerDown(divider(), { pointerId: 1, clientX: 250, clientY: 300 });
        fireEvent.pointerMove(divider(), { pointerId: 1, clientX: 250, clientY: 300 });

        act(() => useFileStore.getState().setCurrentIndex(1));

        // Where the divider sits is not a property of any photograph, and nothing remounts the canvas
        // to make that true - which is what a component keyed on the file would quietly undo.
        expect(divider()).toHaveStyle({ left: "25%" });
        expect(divider()).toHaveAttribute("aria-valuenow", "25");
    });

    it("leaves neither photograph where the divider is not", () => {
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });
        draw("split");

        const before = [drawn(0), drawn(1)];
        fireEvent.pointerDown(divider(), { pointerId: 1, clientX: 250, clientY: 300 });
        fireEvent.pointerMove(divider(), { pointerId: 1, clientX: 250, clientY: 300 });

        // A feature being compared stays at the same point on the screen as the divider passes over
        // it, which is the whole of what makes the comparison readable.
        expect([drawn(0), drawn(1)]).toEqual(before);
    });
});

describe("what the enhanced pane draws", () => {
    it("draws the source in both panes while nothing has been produced", () => {
        // Before a run lands, after one was stopped, and for an image with no enhancements at all -
        // which is the state the canvas has drawn since it had two panes.
        draw("side");

        expect(sources()).toEqual([`opai://localhost/${HOLIDAY.identity}`, `opai://localhost/${HOLIDAY.identity}`]);
    });

    it("points the enhanced pane at the result while the original keeps the source", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        // Which is what makes the two-pane and split comparisons show a comparison for the first
        // time: the original pane goes on drawing the photograph as it was.
        expect(sources()).toEqual([`opai://localhost/${HOLIDAY.identity}`, `opai://localhost/${ENHANCED.identity}`]);
    });

    it("draws the result across the whole canvas in full", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("full");

        expect(sources()).toEqual([`opai://localhost/${ENHANCED.identity}`]);
    });

    it("moves both panes together although they draw different pixels", () => {
        run = { running: false, enhanced: ENHANCED };
        stubPaneGeometry();
        magnify({ scale: 2, x: -100, y: -50 });

        draw("side");

        // The enhanced pane is handed the *source's* identity as its transform key, so it reads and
        // writes the same stored view. Both fit into the same box - every operation this application
        // runs preserves the aspect ratio - so one transform resolves identically in each.
        expect(drawn(0)).toEqual(drawn(1));
        expect(drawn(1)).toEqual({ x: -100, y: -50, scale: 2 });
    });

    it("falls back to the source when a result is withdrawn", () => {
        run = { running: false, enhanced: ENHANCED };
        const view = draw("side");

        expect(sources()[1]).toBe(`opai://localhost/${ENHANCED.identity}`);

        run = { running: false };
        view.rerender(<PreviewImage />);

        expect(sources()[1]).toBe(`opai://localhost/${HOLIDAY.identity}`);
    });
});

/**
 * What the two chips say, which follows the pixels rather than the enhancement stack.
 *
 * The right-hand pane draws the source until a run lands, so calling it *Enhanced* before then would
 * name the photograph by what was asked for instead of by what is on screen.
 */
describe("what the pane labels say", () => {
    const labels = () => [...document.querySelectorAll("[data-slot='preview-chip']")].map((chip) => chip.textContent);

    it("calls both panes the original while nothing has been produced", () => {
        // No enhancements, a first run still working, and a run that was stopped are one state to
        // the canvas: the enhanced pane is pointed at the source, so it is the source it is called.
        draw("side");

        expect(labels()).toEqual(["Original", "Original"]);
    });

    it("goes on calling it the original while the first run is in flight", () => {
        run = { running: true, report: progress() };

        draw("side");

        expect(labels()).toEqual(["Original", "Original"]);
    });

    it("calls the second pane enhanced once a result lands", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        expect(labels()).toEqual(["Original", "Enhanced"]);
    });

    it("keeps the enhanced label while a later run works over a landed result", () => {
        // A result is held until a newer one lands, so the pane is still drawing enhanced pixels -
        // and a chip that fell back to `Original` on every keystroke in the scale field would be
        // describing a photograph that is not on screen.
        run = { running: true, enhanced: ENHANCED, report: progress() };

        draw("side");

        expect(labels()).toEqual(["Original", "Enhanced"]);
    });

    it("calls the one pane in full by whatever it is drawing", () => {
        const view = draw("full");
        expect(labels()).toEqual(["Original"]);
        view.unmount();

        run = { running: false, enhanced: ENHANCED };
        draw("full");
        expect(labels()).toEqual(["Enhanced"]);
    });
});

describe("what the canvas reports about a run", () => {
    it("shows nothing while nothing is running", () => {
        draw("side");

        expect(bar()).toBeNull();
    });

    it("shows an empty bar as soon as a run starts, before it has reported", () => {
        run = { running: true };

        draw("side");

        // A run spends its first seconds loading a model and reports nothing while it does. Waiting
        // for the first report put the bar on screen long after the enhancement was added, and the
        // interval in between read as the application having ignored it.
        expect(bar()).toHaveTextContent("Enhancing");
        expect(filled()).toHaveStyle({ transform: "translateX(-100%)" });
    });

    it("names the enhancement being run, in the interface's own language", () => {
        run = { running: true, fraction: 0.62, report: progress() };

        draw("side");

        // From the report's family rather than from its `operation`, which is the library's composed
        // English sentence and not something a front end can translate.
        expect(bar()).toHaveTextContent("Upscale");
        expect(filled()).toHaveStyle({ transform: "translateX(-38%)" });
    });

    it("reports a fetch with its own percentage instead", () => {
        run = {
            running: true,
            fraction: 0.02,
            report: progress({ stage: "installing", installFraction: 0.41, chainFraction: 0.02 }),
        };

        draw("side");

        // The bar tracks the whole chain, and a fetch occupies only the head of one operation's
        // share of it - so without this number a multi-gigabyte download reads as a stall.
        expect(bar()).toHaveTextContent("Downloading 41%");
        expect(filled()).toHaveStyle({ transform: "translateX(-98%)" });
    });

    it("draws the run's place on the bar rather than the report's own figure", () => {
        // A face recovery is two runs - a detection and then the chain - each reporting its own
        // `0..1`. Drawing `chainFraction` filled the bar, emptied it and filled it again; the hook
        // composes the one figure that spans both, and this is the bar drawing that figure.
        run = { running: true, fraction: 0.2, report: progress({ family: "detection", chainFraction: 1 }) };

        draw("side");

        expect(filled()).toHaveStyle({ transform: "translateX(-80%)" });
    });

    it("names a detection after the enhancement that asked for it", () => {
        // The detector is not an enhancement a user adds and is offered by no menu here, so what the
        // chip says is the enhancement that needed it. This is the first thing a user sees of a face
        // recovery, because the detector is fetched on first use. See design.md D8.
        run = { running: true, report: progress({ family: "detection" }) };

        draw("side");

        expect(bar()).toHaveTextContent("Face Recovery");
    });

    it("names the enhancement generically for a family it has no name for", () => {
        // Drift: a family the library publishes and this front end does not present. It cannot be
        // written as a `Family` because the union names every one it knows, which is the point - the
        // cast is what a renamed family on the wire would actually look like here.
        run = { running: true, report: progress({ family: "segmentation" as Family }) };

        draw("side");

        // The same class of drift `lib/enhancements.ts` answers for a model the catalogue stops
        // publishing, and the same answer: draw something honest rather than throw.
        expect(bar()).toHaveTextContent("Enhancing");
    });

    it("takes the bar down once the run has ended", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        expect(bar()).toBeNull();
    });
});

/*
 * Where the boxes are not.
 *
 * The chooser is the only surface that draws them: the canvas is where a user judges the
 * photograph, and boxes standing over the faces are exactly what stops them from seeing the
 * restoration they asked for. `features/faces/FaceBoxes.tsx` has one production consumer today, so
 * this is a regression test rather than a behaviour one - which is the point, since adding a second
 * consumer here is precisely what it is meant to catch.
 */
describe("the faces in the photograph on the canvas", () => {
    /** One face, shaped as the detector answers them. */
    const face = (left: number): Face => ({
        bounding_box: { min: { x: left, y: 400 }, max: { x: left + 300, y: 700 } },
        landmarks: [
            { x: left + 100, y: 500 },
            { x: left + 200, y: 500 },
            { x: left + 150, y: 600 },
            { x: left + 100, y: 650 },
            { x: left + 200, y: 650 },
        ],
        confidence: 0.9,
    });

    const boxes = () => document.querySelectorAll("[data-slot='face-boxes']");

    beforeEach(() => {
        const identity = HOLIDAY.identity ?? "";

        act(() => {
            useFacesStore.getState().setFaces(identity, undefined, [face(500), face(2000)]);
            useFacesStore.getState().setSkippedFaces(identity, new Set([faceKey(face(500))]));
        });
    });

    afterEach(() => {
        useFacesStore.setState(useFacesStore.getInitialState(), true);
    });

    it("draws no box over the photograph, in any mode", () => {
        for (const mode of ["full", "side", "split"] as const) {
            const view = draw(mode);

            // That the panes are there is what keeps the assertion below from passing because
            // nothing was drawn at all.
            expect(panes().length).toBeGreaterThan(0);
            expect(boxes()).toHaveLength(0);

            view.unmount();
        }
    });

    it("draws no box over the enhanced result either", () => {
        run = { running: false, enhanced: ENHANCED };

        draw("side");

        expect(sources()).toContain(`opai://localhost/${ENHANCED.identity}`);
        expect(boxes()).toHaveLength(0);
    });
});
