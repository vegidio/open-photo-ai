import { act, renderHook, waitFor } from "@testing-library/react";
import type { CropperRef } from "react-advanced-cropper";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CropInfo } from "@/ipc/crop";
import { CROP_DIALOG_BOUND } from "@/lib/constants";
import { track } from "@/lib/faro";
import { useCropStore } from "@/stores/crop";
import { FITTED, useTransformStore } from "@/stores/transform";
import { HOLIDAY, openFiles, resetCropStore, resetFileStore, SUNSET } from "@/test/support";
import { useCropController } from "./useCropController";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// One variable rather than a parameter because `decode` is installed on the prototype: every
// `new Image()` in the module under test comes back with whatever this says, which is how the factor
// is driven from a test without the hook knowing it is being driven.
/** The rendition every preload in this file resolves to, in the widget's own pixels. */
let served = { width: 0, height: 0 };

/**
 * An `HTMLImageElement.decode` whose promises are settled on demand, and a natural size to go with
 * it.
 */
const stubDecode = () => {
    // jsdom implements neither - `decode` is absent entirely and `naturalWidth` is permanently 0 - so a
    // hook that reads the served photograph's dimensions off a preload can be tested no other way. The
    // decode is held open rather than resolved eagerly because one of the cases below is precisely
    // what happens when the dialog is dismissed while it is still in flight.
    const pending: { resolve: () => void; reject: () => void }[] = [];

    // Defined rather than spied on, because `vi.spyOn` refuses a property that is not there - which is
    // also why the `afterEach` takes them away again rather than leaving it to `restoreMocks`.
    Object.defineProperty(HTMLImageElement.prototype, "decode", {
        configurable: true,
        writable: true,
        value: vi.fn(
            () =>
                new Promise<void>((resolve, reject) =>
                    pending.push({ resolve, reject: () => reject(new Error("undecodable")) }),
                ),
        ),
    });

    for (const axis of ["naturalWidth", "naturalHeight"] as const) {
        Object.defineProperty(HTMLImageElement.prototype, axis, {
            configurable: true,
            get: () => (axis === "naturalWidth" ? served.width : served.height),
        });
    }

    return {
        settle: async () => {
            await act(async () => {
                for (const one of pending.splice(0)) one.resolve();
            });
        },
        outstanding: () => pending.length,
    };
};

type Box = { left: number; top: number; width: number; height: number };

/**
 * A stand-in for the widget, behaving the way the pinned version was measured to behave: `imageSize`
 * never moves off the *unrotated* picture, the box lives in the *rotated* one, the clamp **floors**
 * that extent, and `flipImage` is a toggle whose result `transforms.flip` reports.
 *
 * It deliberately does **not** enforce the aspect ratio.
 */
const fakeCropper = (image: { width: number; height: number }) => {
    // Those are the four measurements recorded at the top of `useCropController.ts`, and they are the
    // whole reason a fake is honest here: everything this file asserts is about arithmetic the
    // controller does against those four facts.
    //
    // No aspect ratio, because in the real widget that constraint lives on the stencil, which is a prop
    // rather than anything reachable through the ref, and the controller computes every ratio-driven
    // box itself - so a fake that re-derived the constraint would be checking its own arithmetic
    // instead of the controller's.
    const state = {
        imageSize: { ...image },
        transforms: { rotate: 0, flip: { horizontal: false, vertical: false } },
        coordinates: { left: 0, top: 0, width: image.width, height: image.height } as Box,
    };

    /** The photograph's extent in the rotated space, as `rotateSize` gives it. */
    const extent = () => {
        const radians = (state.transforms.rotate * Math.PI) / 180;
        const cos = Math.abs(Math.cos(radians));
        const sin = Math.abs(Math.sin(radians));

        return {
            width: image.width * cos + image.height * sin,
            height: image.width * sin + image.height * cos,
        };
    };

    /** Rotations applied, so a test can assert the *delta* the controller asked for. */
    const rotations: number[] = [];

    const ref = {
        getState: () => state,
        getCoordinates: (options?: { round?: boolean }) =>
            options?.round
                ? {
                      left: Math.round(state.coordinates.left),
                      top: Math.round(state.coordinates.top),
                      width: Math.round(state.coordinates.width),
                      height: Math.round(state.coordinates.height),
                  }
                : { ...state.coordinates },
        setCoordinates: (next: Box | ((current: typeof state) => Partial<Box>)) => {
            const wanted = typeof next === "function" ? next(state) : next;
            const bounds = extent();

            // Floored, which is the clamp the widget was measured to apply: at 30 degrees an
            // 800x400 photograph reports 892x746 where the real extent is 892.82x746.41.
            const width = Math.min(wanted.width ?? state.coordinates.width, Math.floor(bounds.width));
            const height = Math.min(wanted.height ?? state.coordinates.height, Math.floor(bounds.height));

            state.coordinates = {
                width,
                height,
                left: Math.max(0, Math.min(wanted.left ?? state.coordinates.left, Math.floor(bounds.width) - width)),
                top: Math.max(0, Math.min(wanted.top ?? state.coordinates.top, Math.floor(bounds.height) - height)),
            };
        },
        rotateImage: (delta: number) => {
            rotations.push(delta);
            state.transforms.rotate += delta;
        },
        flipImage: (horizontal: boolean, vertical: boolean) => {
            if (horizontal) state.transforms.flip.horizontal = !state.transforms.flip.horizontal;
            if (vertical) state.transforms.flip.vertical = !state.transforms.flip.vertical;
        },
    };

    return { ref: ref as unknown as CropperRef, state, rotations, extent };
};

/**
 * Lets one animation frame pass, which is what the deferred reshape waits for.
 *
 * Scheduled after the controller's own callback and therefore run after it, which is the ordering
 * jsdom guarantees.
 */
const nextFrame = async () => {
    // The ratio reshape is the one deferral that goes through `requestAnimationFrame` rather than
    // through the effect alone, so a test that asserted straight after the handler would be asserting
    // about the box the user had before they chose anything.
    await act(async () => {
        await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));
    });
};

/** The hook, opened on the current file, with the fake attached the way the widget attaches itself. */
const openOn = async (
    decode: ReturnType<typeof stubDecode>,
    image: { width: number; height: number },
    { ready = true } = {},
) => {
    const widget = fakeCropper(image);
    const hook = renderHook(({ open }: { open: boolean }) => useCropController(open, () => {}), {
        initialProps: { open: true },
    });

    served = image;
    await decode.settle();
    await waitFor(() => expect(hook.result.current.source).toBeDefined());

    hook.result.current.cropper.current = widget.ref;
    if (ready) act(() => hook.result.current.onReady(widget.ref));

    return { hook, widget };
};

describe("useCropController", () => {
    let decode: ReturnType<typeof stubDecode>;

    beforeEach(() => {
        decode = stubDecode();
        resetFileStore();
        resetCropStore();
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    afterEach(() => {
        for (const property of ["decode", "naturalWidth", "naturalHeight"]) {
            Reflect.deleteProperty(HTMLImageElement.prototype, property);
        }
    });

    describe("the photograph it works from", () => {
        it("asks for the uncropped photograph at the dialog's own bound", async () => {
            openFiles(HOLIDAY);

            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            // No `crop=` in it, whatever is recorded: the surface frames the whole photograph.
            expect(hook.result.current.source?.url).toBe(
                `opai://localhost/${HOLIDAY.identity}?size=${CROP_DIALOG_BOUND}`,
            );
        });

        it("is exactly 1 for a photograph the bound does not reduce", async () => {
            openFiles(HOLIDAY);

            // HOLIDAY is 3000x2000, under the 3072 bound, so Rust serves it unchanged - and the
            // factor is not merely close to 1 but exactly it, which is what keeps the common case
            // free of rounding entirely.
            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            expect(hook.result.current.source?.factor).toBe(1);
        });

        it("is the reduction for a photograph the bound does reduce", async () => {
            openFiles(SUNSET);

            // SUNSET is 6000x4000; bounded at 3072 Rust serves 3072x2048.
            const { hook } = await openOn(decode, { width: 3072, height: 2048 });

            expect(hook.result.current.source?.factor).toBe(6000 / 3072);
        });

        it("leaves no state behind when the dialog closes while it is still loading", async () => {
            openFiles(HOLIDAY);
            served = { width: 3000, height: 2000 };

            const hook = renderHook(({ open }: { open: boolean }) => useCropController(open, () => {}), {
                initialProps: { open: true },
            });

            expect(decode.outstanding()).toBe(1);

            hook.rerender({ open: false });
            await decode.settle();

            expect(hook.result.current.source).toBeUndefined();
        });
    });

    describe("a framing already recorded", () => {
        /** A framing using every one of the seven fields, on a photograph the bound does not reduce. */
        const SEEDED: CropInfo = {
            left: 120,
            top: 80,
            width: 1400,
            height: 900,
            millidegrees: 95_000,
            flipHorizontal: true,
            flipVertical: true,
        };

        it("seeds the widget with every field of it", async () => {
            openFiles(HOLIDAY);
            act(() => useCropStore.getState().setCrop(HOLIDAY.identity ?? "", SEEDED));

            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            expect(widget.state.transforms.flip).toEqual({ horizontal: true, vertical: true });
            expect(widget.state.transforms.rotate).toBe(95);
            expect(widget.ref.getCoordinates({ round: true })).toMatchObject({
                left: 120,
                top: 80,
                width: 1400,
                height: 900,
            });

            // 95 degrees is one quarter turn and 5 on the slider, which is the split the two controls
            // hold it in - and is what makes re-opening show the slider where the user left it.
            expect(hook.result.current.fineRotation).toBe(5);
        });

        it("shows its dimensions as recorded rather than round-tripped through the factor", async () => {
            openFiles(SUNSET);
            act(() => useCropStore.getState().setCrop(SUNSET.identity ?? "", { ...SEEDED, width: 2560 }));

            const { hook } = await openOn(decode, { width: 3072, height: 2048 });

            expect(hook.result.current.width).toBe(2560);
        });

        it("puts a counter-clockwise turn back on the slider leaning the way it was applied", async () => {
            openFiles(HOLIDAY);
            act(() => useCropStore.getState().setCrop(HOLIDAY.identity ?? "", { ...SEEDED, millidegrees: -45_000 }));

            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            // No quarter turn and a slider at -45, rather than the quarter turn back and a slider at
            // +45 that flooring the split produces: both describe this picture, and only one of them
            // agrees with the way it visibly leans.
            expect(widget.state.transforms.rotate).toBe(-45);
            expect(hook.result.current.fineRotation).toBe(-45);
        });

        it("reads a turn recorded the long way round as the short one", async () => {
            openFiles(HOLIDAY);
            act(() => useCropStore.getState().setCrop(HOLIDAY.identity ?? "", { ...SEEDED, millidegrees: 350_000 }));

            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            // 350 is 10 counter-clockwise, and the slider says so. Nothing this dialog writes is
            // spelled that way, but the field is an `i32` and Rust turns by it modulo 360, so the
            // split is written to take whatever arrives rather than only its own output.
            expect(hook.result.current.fineRotation).toBe(-10);
        });
    });

    describe("turning and mirroring", () => {
        it("reaches the widget as the angle the slider asked for", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onRotationChange(12));

            expect(widget.state.transforms.rotate).toBe(12);
            expect(hook.result.current.fineRotation).toBe(12);
        });

        it("squares a leaning photograph up rather than adding to the lean", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onRotationChange(7));
            act(() => hook.result.current.onRotate90());

            // 90 rather than 97: the quarter turn snaps to the next multiple and zeroes the slider.
            expect(widget.state.transforms.rotate).toBe(90);
            expect(hook.result.current.fineRotation).toBe(0);
        });

        it("comes back to no turn at all after four quarter turns", async () => {
            openFiles(HOLIDAY);
            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            for (let turn = 0; turn < 4; turn += 1) act(() => hook.result.current.onRotate90());

            // Asserted through what would be recorded rather than through the widget's own `rotate`,
            // which is left free to accumulate: the controller normalises, so the framing says 0.
            act(() => hook.result.current.onApply());

            expect(useCropStore.getState().crops.get(HOLIDAY.identity ?? "")).toBeUndefined();
        });

        it("takes the shortest way round rather than spinning back the long way", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            for (let turn = 0; turn < 3; turn += 1) act(() => hook.result.current.onRotate90());
            act(() => hook.result.current.onReset());

            // The last delta is +90 to reach 360, not -270 back to 0. Either lands the photograph in
            // the same place; only one of them does it without visibly unwinding three quarter turns.
            expect(widget.rotations.at(-1)).toBe(90);
        });

        it("leaves no mirror after the same mirror twice", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onFlipHorizontal());
            expect(widget.state.transforms.flip.horizontal).toBe(true);

            act(() => hook.result.current.onFlipHorizontal());
            expect(widget.state.transforms.flip).toEqual({ horizontal: false, vertical: false });
        });
    });

    describe("choosing an aspect ratio", () => {
        it("lands on the same size every time a ratio is chosen again", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();
            const first = { ...widget.state.coordinates };

            act(() => hook.result.current.onSelectRatio("3:2"));
            await nextFrame();
            expect(widget.state.coordinates.height).not.toBe(first.height);

            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();

            // The divergence from the reference, which fits each new ratio *inside* the current box:
            // there, 16:9 then 3:2 then 16:9 lands at 2531x1424 against this 3000x1687, and nothing
            // but a reset grows it back. HOLIDAY is itself 3:2, so it is the *height* that carries
            // the assertion - both ratios reach the photograph's full width.
            expect(widget.state.coordinates).toEqual(first);
        });

        it("measures against the turned photograph rather than the unturned one", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onRotate90());
            act(() => hook.result.current.onSelectRatio("square"));
            await nextFrame();

            // Turned a quarter, a 3000x2000 photograph is 2000 wide and 3000 tall, so the largest
            // square that fits it is 2000. Measured against `imageSize` - the unturned picture, which
            // is what the reference's own reset uses - the answer would be the same 2000 by accident;
            // it is the *height* that gives the game away, since `imageSize.height` is 2000 and the
            // turned extent is 3000, leaving room for the square to be placed anywhere down it.
            // Through the rounded reading, which is the one the controller and the fields use: a
            // quarter turn goes through `Math.cos`, so the raw extent is 2000.0000000000002.
            expect(widget.ref.getCoordinates({ round: true })).toMatchObject({ width: 2000, height: 2000 });
            expect(Math.floor(widget.extent().height)).toBe(3000);
        });
    });

    describe("typing a dimension", () => {
        it("gives the height the locked ratio implies", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();

            act(() => hook.result.current.onWidthCommit(1600));

            expect(widget.state.coordinates.width).toBe(1600);
            expect(widget.state.coordinates.height).toBe(900);
        });

        it("brings a width larger than the photograph inside it", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onWidthCommit(9999));

            expect(widget.state.coordinates.width).toBe(3000);
        });

        it("is read and reported in the photograph's own pixels, not the rendition's", async () => {
            openFiles(SUNSET);
            const { hook, widget } = await openOn(decode, { width: 3072, height: 2048 });

            // SUNSET is 6000 wide served at 3072, so the factor is 1.953125: typing 2560 of the
            // photograph's pixels is 1310.72 of the widget's, which it rounds to 1311.
            act(() => hook.result.current.onWidthCommit(2560));

            expect(widget.state.coordinates.width).toBe(1311);
            // And back out again, which is the one-pixel snap `CropDimensions` contains to the blur.
            expect(hook.result.current.width).toBe(2561);
        });
    });

    describe("swapping the two dimensions", () => {
        it("selects the paired ratio and keeps the lock", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();
            const before = { ...widget.state.coordinates };

            await act(async () => hook.result.current.onSwap());

            // The divergence from the reference, which drops to Free here and leaves the grid marking
            // "no constraint" beside a box that is exactly 9:16.
            expect(hook.result.current.ratio).toBe("9:16");
            expect(hook.result.current.aspectRatio).toBe(9 / 16);
            expect(widget.state.coordinates.width).toBe(before.height);
        });

        it("transposes and stays free when nothing is locked", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onWidthCommit(1200));
            act(() => hook.result.current.onHeightCommit(800));
            await act(async () => hook.result.current.onSwap());

            expect(hook.result.current.ratio).toBe("free");
            expect(hook.result.current.aspectRatio).toBeUndefined();
            expect(widget.state.coordinates).toMatchObject({ width: 800, height: 1200 });
        });
    });

    describe("reset", () => {
        it("returns the turn, the mirrors, the lock and the box to their defaults", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onRotate90());
            act(() => hook.result.current.onRotationChange(12));
            act(() => hook.result.current.onFlipVertical());
            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();
            act(() => hook.result.current.onWidthCommit(900));

            await act(async () => hook.result.current.onReset());

            expect(hook.result.current.ratio).toBe("free");
            expect(hook.result.current.aspectRatio).toBeUndefined();
            expect(hook.result.current.fineRotation).toBe(0);
            expect(widget.state.transforms.flip).toEqual({ horizontal: false, vertical: false });
            expect(widget.state.coordinates).toMatchObject({ left: 0, top: 0, width: 3000, height: 2000 });
        });

        it("leaves a recorded framing untouched when the surface is then dismissed", async () => {
            const recorded: CropInfo = {
                left: 10,
                top: 20,
                width: 500,
                height: 400,
                millidegrees: 0,
                flipHorizontal: false,
                flipVertical: false,
            };

            openFiles(HOLIDAY);
            act(() => useCropStore.getState().setCrop(HOLIDAY.identity ?? "", recorded));

            const { hook } = await openOn(decode, { width: 3000, height: 2000 });
            await act(async () => hook.result.current.onReset());

            // Reset is a control, not an action on the image: nothing outside the surface moves until
            // Apply, so dismissing after a reset is dismissing.
            expect(useCropStore.getState().crops.get(HOLIDAY.identity ?? "")).toEqual(recorded);
        });
    });

    describe("applying", () => {
        it("records every field scaled to the photograph's own pixels", async () => {
            openFiles(SUNSET);
            const { hook, widget } = await openOn(decode, { width: 3072, height: 2048 });

            act(() => hook.result.current.onRotationChange(12));
            act(() => hook.result.current.onFlipHorizontal());
            act(() => widget.ref.setCoordinates({ left: 100, top: 200, width: 1000, height: 800 }));
            act(() => hook.result.current.onApply());

            const factor = 6000 / 3072;

            expect(useCropStore.getState().crops.get(SUNSET.identity ?? "")).toEqual({
                left: Math.round(100 * factor),
                top: Math.round(200 * factor),
                width: Math.round(1000 * factor),
                height: Math.round(800 * factor),
                // Whole degrees on the slider, so the millidegree unit is always exact here - it buys
                // room this dialog does not spend.
                millidegrees: 12_000,
                flipHorizontal: true,
                flipVertical: false,
            });
            // What the framing does, never where or how large.
            expect(track).toHaveBeenCalledExactlyOnceWith("crop_applied", {
                rotated: true,
                flipped: true,
                has_ratio: false,
            });
        });

        it("clears a recorded framing rather than storing one that frames everything", async () => {
            const recorded: CropInfo = {
                left: 10,
                top: 20,
                width: 500,
                height: 400,
                millidegrees: 0,
                flipHorizontal: false,
                flipVertical: false,
            };

            openFiles(HOLIDAY);
            act(() => useCropStore.getState().setCrop(HOLIDAY.identity ?? "", recorded));

            const { hook } = await openOn(decode, { width: 3000, height: 2000 });
            await act(async () => hook.result.current.onReset());
            act(() => hook.result.current.onApply());

            expect(useCropStore.getState().crops.has(HOLIDAY.identity ?? "")).toBe(false);
        });

        it("records a counter-clockwise turn as the angle the user set, and re-opens on it", async () => {
            openFiles(HOLIDAY);
            const first = await openOn(decode, { width: 3000, height: 2000 });

            act(() => first.hook.result.current.onRotationChange(-45));
            act(() => first.hook.result.current.onApply());

            // -45, not the 315 that wrapping into [0, 360) would record. The two turn the photograph
            // to the same place, and only one of them can be split back into the quarter turns and
            // the slider position that produced it.
            expect(useCropStore.getState().crops.get(HOLIDAY.identity ?? "")?.millidegrees).toBe(-45_000);

            const second = await openOn(decode, { width: 3000, height: 2000 });

            expect(second.widget.state.transforms.rotate).toBe(-45);
            expect(second.hook.result.current.fineRotation).toBe(-45);

            // And the slider still means what it says: levelling it puts the photograph upright,
            // rather than turning it the quarter that a base of 270 would have been hiding behind it.
            act(() => second.hook.result.current.onRotationChange(0));

            expect(second.widget.state.transforms.rotate).toBe(0);
        });

        it("says a ratio was kept when one is locked", async () => {
            openFiles(HOLIDAY);
            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onSelectRatio("16:9"));
            await nextFrame();
            act(() => hook.result.current.onApply());

            expect(track).toHaveBeenCalledExactlyOnceWith("crop_applied", {
                rotated: false,
                flipped: false,
                has_ratio: true,
            });
        });

        it("leaves an unframed photograph unframed", async () => {
            openFiles(HOLIDAY);
            const { hook } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onApply());

            expect(useCropStore.getState().crops.size).toBe(0);
            expect(track).not.toHaveBeenCalled();
        });

        it("refits the canvas for the photograph whose shape changed, and for no other", async () => {
            // `addFiles` makes the first of a batch current, so HOLIDAY is the one being framed and
            // SUNSET is the other image the refit must not reach.
            openFiles(HOLIDAY, SUNSET);

            const elsewhere = { scale: 4, x: -100, y: -50 };
            act(() => useTransformStore.getState().setTransform(SUNSET.identity ?? "", elsewhere));
            act(() => useTransformStore.getState().setTransform(HOLIDAY.identity ?? "", elsewhere));

            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });
            act(() => widget.ref.setCoordinates({ left: 0, top: 0, width: 1000, height: 800 }));
            act(() => hook.result.current.onApply());

            expect(useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "")).toEqual(FITTED);
            expect(useTransformStore.getState().transforms.get(SUNSET.identity ?? "")).toMatchObject(elsewhere);
        });

        it("leaves a magnified canvas alone when the framing did not change", async () => {
            openFiles(HOLIDAY);

            const examining = { scale: 4, x: -100, y: -50 };
            act(() => useTransformStore.getState().setTransform(HOLIDAY.identity ?? "", examining));

            const { hook } = await openOn(decode, { width: 3000, height: 2000 });
            act(() => hook.result.current.onApply());

            // Opening the surface, looking and applying is how a user backs out of framing - and
            // `gui-crop` says nothing about how the image is drawn changes, so the detail they were
            // examining at 4x is still what they come back to.
            expect(useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "")).toMatchObject(examining);
        });

        it("writes nothing to either store when the surface is merely dismissed", async () => {
            openFiles(HOLIDAY);
            const { hook, widget } = await openOn(decode, { width: 3000, height: 2000 });

            act(() => hook.result.current.onRotate90());
            act(() => widget.ref.setCoordinates({ left: 0, top: 0, width: 900, height: 700 }));

            // No Apply.
            expect(useCropStore.getState().crops.size).toBe(0);
            expect(useTransformStore.getState().transforms.size).toBe(0);
        });
    });
});
