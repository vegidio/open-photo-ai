import { useCallback, useEffect, useRef, useState } from "react";
import { type Coordinates, type CropperRef, type CropperState, rotateSize } from "react-advanced-cropper";
import { FREE_RATIO, ratioByKey } from "@/features/crop/ratios";
import type { CropInfo } from "@/ipc/crop";
import { renditionFor } from "@/ipc/images";
import { CROP_DIALOG_BOUND, MIN_CROP_SIZE } from "@/lib/constants";
import { track } from "@/lib/faro";
import { clampTo } from "@/lib/utils";
import { useCropStore, useImageCrop } from "@/stores/crop";
import { useCurrentFile } from "@/stores/files";
import { FITTED, useTransformStore } from "@/stores/transform";

// What the widget was measured to do, in the version this project pins.
//
// Measured rather than assumed, in a scratch vite page driven by
// `chrome --headless --dump-dom --virtual-time-budget` against `react-advanced-cropper` 0.20.1,
// because three of the decisions below depend on behaviour no test in this repository can reach:
// jsdom lays nothing out. The numbers are the ones the page printed.
//
// 1. **`getState().imageSize` is the *unrotated* picture; the coordinates are in the *rotated* one.**
//    An 800x400 photograph reports `imageSize` of 800x400 at 0, at 30 and at 90 degrees and under a
//    flip - it never moves. Asking for an impossibly large box at 90 degrees clamps to 400x800,
//    which is the rotated extent. This is what `extentOf` exists for, and it is why the
//    reference's `onReset` - which resets to `imageSize` - under-sizes a turned photograph.
//
// 2. **`rotateSize` is term-for-term the formula `rust_sak::image::rotate` implements.**
//    `rotateSize({ 800, 400 }, 30)` answers `892.820323027551 x 746.4101615137754`, which is
//    `w|cos t| + h|sin t|` by `w|sin t| + h|cos t|` to the last digit; 12 degrees agrees the same
//    way. The widget's integer clamp then *floors* that extent - 892x746 - where Rust's `rotate`
//    rounds it outward to 893x747. A rectangle taken from the widget is therefore always strictly
//    inside the canvas Rust produces, which is the direction this needs to be wrong in.
//
// 3. **`getCoordinates({ round: true })` stays in the rotated space under a flip as well.**
//    With `flip.horizontal` set *and* a quarter turn applied, the extent is still 400x800. The flip
//    does not re-base the coordinates, so nothing here has to compensate for one.
//
// 4. **`transforms.flip` is the truth about the mirrors, and `flipImage` is a toggle.**
//    `flipImage(true, false)` once leaves `{ horizontal: true, vertical: false }`; a second call
//    leaves `{ horizontal: false, vertical: false }`. That is why the mirrors are not mirrored into
//    React state here - see `useCropController`.

/** A whole turn, in degrees. */
const FULL_TURN = 360;

/** The quarter-turn the dedicated action steps by. */
const ROTATE_STEP = 90;

/** An angle wrapped into `[0, 360)`, which is the range the quarter turns are held in. */
const normalizeAngle = (degrees: number) => ((degrees % FULL_TURN) + FULL_TURN) % FULL_TURN;

/** An angle wrapped into `(-180, 180]`, which is the range `CropInfo` records a turn in. */
const signedAngle = (degrees: number) => {
    // **Signed rather than `[0, 360)`, because the sign is the user's and is not recoverable once it
    // is gone.** A photograph levelled by -45 degrees and one turned three quarters and then +45 are
    // the same picture and both normalise to 315, so a recording in `[0, 360)` cannot be split back
    // into the controls that produced it: `splitAngle` would have to guess, and a photograph levelled
    // by -45 would re-open with a slider at +45 beside a picture visibly leaning the other way - and
    // then turn a quarter when that slider was dragged back to zero. Wrapping to the shortest signed
    // turn instead records every angle the slider can reach as itself.
    //
    // Rust takes it as it comes: `millidegrees` is an `i32`, the `crop` parameter parses a negative
    // field like any other, and `rust_sak::image::rotate` takes its angle modulo 360.
    const wrapped = normalizeAngle(degrees);

    return wrapped > FULL_TURN / 2 ? wrapped - FULL_TURN : wrapped;
};

// `Math.floor` rather than a round, so the step always lands on the *next* multiple of 90. It is not
// what splits a recorded angle back into the two controls - see `splitAngle`, which has a sign to
// carry that this deliberately does not.
/**
 * An angle snapped down to the quarter turn at or below it, which is what the quarter-turn action
 * steps from.
 */
const snapToStep = (degrees: number) => Math.floor(degrees / ROTATE_STEP) * ROTATE_STEP;

/**
 * A recorded angle split back into the quarter turns and the slider that produced it.
 *
 * `fine` keeps the angle's sign and lands in `(-90, 90)` - inside the slider's own `[-90, 90]` - for
 * every angle {@link signedAngle} produces. `base` is normalised into `[0, 360)`, the range the
 * quarter turns are held in everywhere else.
 */
const splitAngle = (degrees: number) => {
    // Truncated **towards zero** rather than floored, which is the half of this that carries the sign:
    // -45 degrees is no quarter turn and a slider at -45, where flooring calls it a quarter turn back
    // and a slider at +45 - the same picture, described by controls that disagree with it. A truncated
    // remainder keeps its dividend's sign.
    const base = Math.trunc(degrees / ROTATE_STEP) * ROTATE_STEP;

    // Normalising `base` loses nothing: it reaches the widget through `applyRotation`, which goes the
    // shortest way round and so cannot tell 270 from -90.
    return { base: normalizeAngle(base), fine: degrees - base };
};

// Measurement 1 above applied: `imageSize` alone is the unrotated picture and under-sizes a turned
// one, and the box lives in the turned space. This is the one expression the ratio fit and the reset
// both measure against, so neither can come to disagree with the other about how big the photograph
// is.
/**
 * The extent of the photograph in the widget's own coordinate space - the space the box is in:
 * `imageSize` turned by whatever the widget is currently turned by.
 */
const extentOf = (state: CropperState) => rotateSize(state.imageSize, state.transforms.rotate);

/** What the dialog has loaded to work from: the reduced photograph, and what converts back to the real one. */
type Source = {
    /** Where the widget draws from - the *uncropped* photograph, bounded at {@link CROP_DIALOG_BOUND}. */
    url: string;
    /**
     * What one of the widget's pixels is worth in the photograph's own, at least 1.
     *
     * `source.longestEdge / rendition.longestEdge`, read off the rendition the widget actually
     * loaded rather than assumed from the bound - Rust never enlarges, so a photograph under the
     * bound comes back unchanged and this is exactly 1, which is the common case and is exact.
     */
    factor: number;
};

/**
 * Everything the Crop/Rotate dialog is, less its layout.
 *
 * The components under `features/crop/` are the surface; this is the state, the effects and the
 * handlers behind them.
 */
export const useCropController = (open: boolean, onClose: () => void) => {
    // The split from the components is the reference's `useCropController` and is taken for its stated
    // reason: the effect ordering below is the hard part of this dialog, and spreading it across five
    // components would put each half of it out of sight of the other.
    const file = useCurrentFile();
    const identity = file?.identity;
    const recorded = useImageCrop(identity);
    const setCrop = useCropStore((state) => state.setCrop);
    const forgetCrop = useCropStore((state) => state.forgetCrop);
    const setTransform = useTransformStore((state) => state.setTransform);

    const cropper = useRef<CropperRef>(null);

    const [source, setSource] = useState<Source>();
    const [ratio, setRatio] = useState(FREE_RATIO);
    // Derived rather than held: every write to the lock is a write to the ratio it is the value of.
    const aspectRatio = ratioByKey(ratio)?.value;

    // **The rotation is held here**, in two pieces, because the two controls that write it are two
    // different gestures: `baseRotation` is the quarter turns and `fineRotation` is the slider. The
    // widget is then driven to their sum by the shortest-path delta from wherever it currently is - see
    // `applyRotation`.
    //
    // **The mirrors are not.** `flipImage` is a toggle and the widget's own `transforms.flip` is what
    // it acted on (measurement 4), so a pair of booleans beside it would be a second source of truth
    // that reset would have to keep in step. Reset reads the flips back off the widget to decide what
    // to undo, exactly as the reference does.
    const [baseRotation, setBaseRotation] = useState(0);
    const [fineRotation, setFineRotation] = useState(0);
    const [width, setWidth] = useState(0);
    const [height, setHeight] = useState(0);

    /*
     * The two deferrals, each a piece of pending state with an effect behind it.
     *
     * Neither can be done inline in its handler, because each one sets a box *and* changes
     * the lock the stencil will constrain that box by, and on the render that changes the lock the
     * stencil is still holding the previous one. Written inline, the new box is re-constrained to the
     * ratio the user just left.
     *
     * They are two rather than one, and that is load-bearing rather than tidy: a swap changes
     * `aspectRatio` to the *inverse* ratio, so a reshape effect keyed on `aspectRatio` itself would
     * fire for a swap as well and re-fit the box to the largest rectangle of the new ratio - which is
     * the one thing a swap must not do. Keying the reshape on its own flag is what keeps "the user
     * chose a ratio" and "the user swapped" two different events.
     */
    const [pendingReshape, setPendingReshape] = useState(false);
    // A swap's box and a reset's are one deferral: each is a box computed from the widget's state once
    // the changed lock has reached the stencil. Wrapped so that `useState` holds the function rather
    // than calling it.
    const [pendingBox, setPendingBox] = useState<{ at: (state: CropperState) => Coordinates }>();

    /** The ratio and the rotation back where the dialog starts them. */
    const resetControls = useCallback(() => {
        setRatio(FREE_RATIO);
        setBaseRotation(0);
        setFineRotation(0);
    }, []);

    /*
     * The photograph as the dialog will draw it, loaded when the dialog opens and dropped when it
     * closes.
     *
     * **Uncropped, whatever framing is recorded**, which is the surface's own contract: a framing is
     * a choice about the whole photograph and has to stay reversible, and a surface that opened on
     * the already-framed picture could only ever cut further into it.
     *
     * The decode is waited for rather than handed straight to the widget because the factor is read
     * off it: `naturalWidth` is what Rust actually served, and that is the only honest divisor. The
     * preload is the same one `hooks/useSettledFile.ts` uses on the canvas, including its feature
     * test - jsdom implements no `decode`, and a webview old enough to lack it is served by the
     * `load` event just as well here, since nothing is being painted at this point.
     */
    useEffect(() => {
        if (!open) {
            setSource(undefined);
            return;
        }

        const url = renditionFor(file, CROP_DIALOG_BOUND);
        if (url === undefined) return;

        let live = true;

        const settle = (image: HTMLImageElement) => {
            if (!live) return;

            /*
             * Both edges of both pictures, because which one is longest is not knowable in advance -
             * a turned rendition is not a thing Rust serves, but a photograph whose header was
             * unreadable is, and it arrives with no dimensions at all. Falling back to 1 for that
             * case is the only answer available: the application could not measure the photograph,
             * so it cannot say what the rendition is a reduction of. It is also harmless, because
             * such a file has no dimensions for the navbar or the canvas either.
             */
            const drawn = Math.max(image.naturalWidth, image.naturalHeight);
            const whole = Math.max(file?.width ?? 0, file?.height ?? 0);
            const factor = drawn > 0 && whole > 0 ? whole / drawn : 1;

            setSource({ url, factor });
        };

        const preload = new Image();

        if (typeof preload.decode !== "function") {
            preload.addEventListener("load", () => settle(preload));
            preload.addEventListener("error", () => live && setSource({ url, factor: 1 }));
            preload.src = url;
        } else {
            preload.src = url;
            preload.decode().then(
                () => settle(preload),
                // A rendition the protocol refused still has to reach the widget, which draws it as a
                // broken image - the same answer the canvas gives, rather than a dialog that never
                // opens. There is nothing to measure, so the factor is the identity.
                () => live && setSource({ url, factor: 1 }),
            );
        }

        return () => {
            live = false;
        };
    }, [open, file]);

    /*
     * Back to the defaults whenever the dialog opens on a photograph. The widget itself is seeded
     * from any recorded framing in `onReady`, which is the only moment it is ready to be driven.
     */
    useEffect(() => {
        if (!open) return;

        resetControls();
        setPendingReshape(false);
        setPendingBox(undefined);
    }, [open, resetControls]);

    /** One of the widget's lengths as the photograph's own, which is what the dimension fields show. */
    const toSource = useCallback((value: number) => Math.round(value * (source?.factor ?? 1)), [source?.factor]);

    /** One of the photograph's lengths as the widget's, which is what a typed dimension is read as. */
    const toWidget = useCallback((value: number) => value / (source?.factor ?? 1), [source?.factor]);

    /** The live box, in the photograph's own pixels, which is what the fields mirror while unfocused. */
    const syncDimensions = useCallback(
        (instance: CropperRef) => {
            const box = instance.getCoordinates({ round: true });
            if (!box) return;

            setWidth(toSource(box.width));
            setHeight(toSource(box.height));
        },
        [toSource],
    );

    /**
     * Turns the widget to an absolute angle by applying the delta from wherever it currently is.
     *
     * The delta is normalised to the shortest path, so 0 and 360 are the same instruction and produce
     * no movement, and a reset from 350 degrees walks back 10 rather than forward 350.
     *
     * `immediate` is for the slider's own drag: a transition per pointer event fights the gesture it
     * is meant to be tracking.
     */
    const applyRotation = useCallback((target: number, immediate = false) => {
        // The reference's `applyRotation`, carried over whole.
        const current = cropper.current?.getState()?.transforms.rotate ?? 0;
        let delta = normalizeAngle(target - current);
        if (delta > FULL_TURN / 2) delta -= FULL_TURN;

        cropper.current?.rotateImage(delta, immediate ? { transitions: false } : undefined);
    }, []);

    /**
     * Seeds the widget from a recorded framing, once it is ready to be driven, and splits the
     * recorded angle back into the two controls that produced it.
     *
     * With nothing recorded the widget keeps its own default - the whole photograph, which is what
     * `defaultSize` asks for - and the fields are filled from it.
     */
    const onReady = useCallback(
        (instance: CropperRef) => {
            if (!recorded) {
                syncDimensions(instance);
                return;
            }

            if (recorded.flipHorizontal || recorded.flipVertical) {
                instance.flipImage(recorded.flipHorizontal, recorded.flipVertical);
            }

            const degrees = signedAngle(recorded.millidegrees / 1000);
            if (degrees !== 0) instance.rotateImage(degrees, { transitions: false });

            // Divided by the factor on the way in for the same reason it is multiplied on the way out:
            // what was recorded is in the photograph's pixels, and the widget works in the rendition's.
            instance.setCoordinates({
                left: toWidget(recorded.left),
                top: toWidget(recorded.top),
                width: toWidget(recorded.width),
                height: toWidget(recorded.height),
            });

            const { base, fine } = splitAngle(degrees);
            setBaseRotation(base);
            setFineRotation(fine);

            // From the recording rather than from the widget: these are already the photograph's own
            // pixels, and round-tripping them through the factor would show a value a pixel off the
            // one the user applied.
            setWidth(recorded.width);
            setHeight(recorded.height);
        },
        [recorded, toWidget, syncDimensions],
    );

    /*
     * A ratio the user chose from the grid: the largest rectangle of it that fits the photograph,
     * centred on the box they had.
     *
     * Against the *rotated* extent rather than `imageSize`, which is measurement 1 applied - on a
     * turned photograph `imageSize` is the smaller, unturned picture and would fit the new ratio
     * inside a box smaller than the space available.
     *
     * Against the photograph rather than inside the current box, which is the divergence from the
     * reference: fitting inside `coords` means every choice can only shrink the selection, so 16:9
     * then 3:2 then 16:9 lands smaller than 16:9 did the first time and nothing but a reset gets it
     * back.
     *
     * Deferred a frame because on the render that sets the ratio the new `aspectRatio` has not
     * reached the stencil, and the box would be re-constrained to the previous one. The applied size
     * is then pushed into the fields by hand: the widget fires no `onChange` for a reshape it did
     * not initiate, so they would otherwise go on showing the old box.
     */
    useEffect(() => {
        if (!pendingReshape) return;

        const frame = requestAnimationFrame(() => {
            const instance = cropper.current;
            setPendingReshape(false);
            if (!instance) return;

            if (aspectRatio !== undefined) {
                instance.setCoordinates((state) => {
                    const extent = extentOf(state);
                    const box = state.coordinates ?? { left: 0, top: 0, ...extent };
                    const centreX = box.left + box.width / 2;
                    const centreY = box.top + box.height / 2;

                    const fitted =
                        extent.width / extent.height > aspectRatio ? extent.height * aspectRatio : extent.width;

                    return {
                        width: fitted,
                        height: fitted / aspectRatio,
                        left: centreX - fitted / 2,
                        top: centreY - fitted / aspectRatio / 2,
                    };
                });
            }

            syncDimensions(instance);
        });

        return () => cancelAnimationFrame(frame);
    }, [pendingReshape, aspectRatio, syncDimensions]);

    /*
     * A swap's or a reset's box, once the changed lock has reached the stencil.
     *
     * A swap does not clear the lock but replaces it with the paired ratio, and a reset clears it; in
     * both the box has to be set against the stencil's new constraint, not its old one. A plain effect
     * rather than an animation frame, as the reference does it: by the time an effect runs the render
     * has been committed and the stencil holds the new prop, which is the condition the deferral is
     * about.
     */
    useEffect(() => {
        if (!pendingBox) return;

        const instance = cropper.current;
        instance?.setCoordinates(pendingBox.at);

        setPendingBox(undefined);
        if (instance) syncDimensions(instance);
    }, [pendingBox, syncDimensions]);

    /** The slider: a fine rotation on top of whatever quarter turns are in force. */
    const onRotationChange = useCallback(
        (value: number) => {
            setFineRotation(value);
            applyRotation(baseRotation + value, true);
        },
        [applyRotation, baseRotation],
    );

    /** The quarter-turn action: to the next multiple of 90, with the fine rotation zeroed. */
    const onRotate90 = useCallback(() => {
        const next = snapToStep(baseRotation + fineRotation) + ROTATE_STEP;

        // `normalizeAngle` on the way into state is what keeps four turns arriving back at 0 rather
        // than at 360.
        setBaseRotation(normalizeAngle(next));
        // Zeroing the slider is the reference's behaviour and is kept deliberately: a user who has
        // levelled a photograph by 7 degrees and then turns it a quarter is squaring it up, not asking
        // for 97.
        setFineRotation(0);
        applyRotation(next);
    }, [applyRotation, baseRotation, fineRotation]);

    const onFlipHorizontal = useCallback(() => cropper.current?.flipImage(true, false), []);

    const onFlipVertical = useCallback(() => cropper.current?.flipImage(false, true), []);

    /** A ratio chosen from the grid. The reshape itself is the deferred effect above. */
    const onSelectRatio = useCallback((key: string) => {
        setRatio(key);
        setPendingReshape(true);
    }, []);

    /**
     * The swap: the box transposed, and the lock moved to the paired ratio rather than dropped.
     *
     * For Free and Square, which are their own inverses, the mark does not move.
     */
    const onSwap = useCallback(() => {
        const box = cropper.current?.getCoordinates();
        if (!box) return;

        // The reference transposes and drops to Free, which throws away the constraint the user chose
        // and leaves the grid marking Free beside a box that is exactly 9:16. Every option in the table
        // carries its own inverse, so there is always somewhere to go.
        const paired = ratioByKey(ratioByKey(ratio)?.inverse ?? FREE_RATIO);

        setRatio(paired?.key ?? FREE_RATIO);
        setPendingBox({
            at: (state) => ({
                left: state.coordinates?.left ?? 0,
                top: state.coordinates?.top ?? 0,
                width: box.height,
                height: box.width,
            }),
        });
    }, [ratio]);

    /** Reset: every control back to where it started, and nothing outside the dialog touched. */
    const onReset = useCallback(() => {
        resetControls();
        applyRotation(0);

        // The mirrors are undone by toggling exactly the ones the widget reports as on (measurement 4).
        const flip = cropper.current?.getState()?.transforms.flip;
        if (flip?.horizontal || flip?.vertical) cropper.current?.flipImage(flip.horizontal, flip.vertical);

        // The box waits for the cleared lock. The whole photograph, measured as `extentOf` measures it
        // everywhere else - which at this point is the unturned picture, because the rotation is already
        // back at zero, but is written in terms of the extent so that the two never diverge.
        setPendingBox({ at: (state) => ({ left: 0, top: 0, ...extentOf(state) }) });
    }, [applyRotation, resetControls]);

    /**
     * A typed dimension, in the photograph's own pixels.
     *
     * Clamped in the *widget's* space - to the rotated extent and to `MIN_CROP_SIZE`, whose docs say
     * why that space is the conservative one. Under a lock the other dimension follows from the
     * ratio; without one it is left exactly as it was.
     */
    const commit = useCallback(
        (axis: "width" | "height", value: number) => {
            const instance = cropper.current;
            const state = instance?.getState();
            if (!instance || !state) return;

            const extent = extentOf(state);
            const clamp = (length: number, limit: number) => clampTo(Math.round(length), MIN_CROP_SIZE, limit);

            const wanted = clamp(toWidget(value), axis === "width" ? extent.width : extent.height);

            const box =
                axis === "width"
                    ? {
                          width: wanted,
                          height: aspectRatio
                              ? clamp(wanted / aspectRatio, extent.height)
                              : (instance.getCoordinates()?.height ?? wanted),
                      }
                    : {
                          height: wanted,
                          width: aspectRatio
                              ? clamp(wanted * aspectRatio, extent.width)
                              : (instance.getCoordinates()?.width ?? wanted),
                      };

            instance.setCoordinates((current) => ({
                left: current.coordinates?.left ?? 0,
                top: current.coordinates?.top ?? 0,
                ...box,
            }));

            syncDimensions(instance);
        },
        [aspectRatio, toWidget, syncDimensions],
    );

    const onWidthCommit = useCallback((value: number) => commit("width", value), [commit]);

    const onHeightCommit = useCallback((value: number) => commit("height", value), [commit]);

    /**
     * Apply: one write to the crop store, and a refit of the canvas that follows from it.
     *
     * An unchanged framing **clears** any recorded one rather than storing an identity framing.
     * **The canvas is refitted only when the framing actually changed.**
     */
    const onApply = useCallback(() => {
        const instance = cropper.current;
        const state = instance?.getState();
        const box = instance?.getCoordinates({ round: true });
        if (!instance || !state || !box || !identity) return;

        const rotation = signedAngle(baseRotation + fineRotation);
        const flip = state.transforms.flip;

        // Compared against the identity framing **in the widget's own space**, before the factor is
        // applied. That comparison is exact there and would not be after scaling: a box the widget
        // reports as the whole of a 2560-wide rendition multiplies to 7910 of a 7911-wide photograph,
        // and a framing that changes nothing would be recorded as one that does. It is also
        // `imageSize` that it compares against rather than `extentOf`, which is the same value
        // wherever it matters - an unchanged framing is by definition unturned.
        const unchanged =
            rotation === 0 &&
            !flip.horizontal &&
            !flip.vertical &&
            box.left === 0 &&
            box.top === 0 &&
            box.width === state.imageSize.width &&
            box.height === state.imageSize.height;

        const info: CropInfo = {
            left: toSource(box.left),
            top: toSource(box.top),
            width: toSource(box.width),
            height: toSource(box.height),
            millidegrees: Math.round(rotation * 1000),
            flipHorizontal: flip.horizontal,
            flipVertical: flip.vertical,
        };

        // Whether the picture the rest of the application draws is about to be a different shape,
        // which is the whole of what the refit below is gated on.
        const changed = unchanged ? recorded !== undefined : !sameFraming(recorded, info);

        // Opening the dialog, looking and applying is how a user backs out of framing, and a crop that
        // frames everything would mint a new identity for the same pixels and re-run a chain that had
        // already been computed. A framing equal to the recorded one is not written again for the same
        // reason: the faces and the result are keyed on the crop object, so a new one would re-run both.
        if (unchanged) forgetCrop(identity);
        else if (changed) setCrop(identity, info);

        // The condition is not decoration: `gui-images` says an image *whose framing changes* is fitted
        // again, and `gui-crop` says applying a framing that changes nothing changes nothing about how
        // the image is drawn - so a user who was examining a detail at 4x, opened the dialog and
        // applied without touching anything must come back to that detail. Nothing else in the
        // application can know a shape changed, which is why it is this line's job.
        if (changed) setTransform(identity, FITTED);

        // What cropping is used for, and only for an Apply that changed something: never where on the
        // photograph, or how large.
        if (changed) {
            track("crop_applied", {
                rotated: rotation !== 0,
                flipped: flip.horizontal || flip.vertical,
                has_ratio: ratio !== FREE_RATIO,
            });
        }

        onClose();
    }, [baseRotation, fineRotation, identity, recorded, ratio, toSource, forgetCrop, setCrop, setTransform, onClose]);

    return {
        cropper,
        source,
        ratio,
        aspectRatio,
        fineRotation,
        width,
        height,
        onReady,
        syncDimensions,
        onRotationChange,
        onRotate90,
        onFlipHorizontal,
        onFlipVertical,
        onReset,
        onSelectRatio,
        onWidthCommit,
        onHeightCommit,
        onSwap,
        onApply,
    };
};

// Field by field rather than by the identity `crates/gui/src/images/crop.rs` hashes, which is Rust's
// and is not on this side of the boundary. `CropInfo` is seven scalars, so this is the whole of it.
/** Whether two framings describe the same picture. */
const sameFraming = (left: CropInfo | undefined, right: CropInfo) =>
    left !== undefined &&
    left.left === right.left &&
    left.top === right.top &&
    left.width === right.width &&
    left.height === right.height &&
    left.millidegrees === right.millidegrees &&
    left.flipHorizontal === right.flipHorizontal &&
    left.flipVertical === right.flipVertical;
