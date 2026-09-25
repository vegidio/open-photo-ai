import { type CSSProperties, type PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronsLeftRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useEnhancementRun } from "@/hooks/useEnhancementRun";
import { useResizeObserver } from "@/hooks/useResizeObserver";
import { useSettledFile } from "@/hooks/useSettledFile";
import { framedDimensions } from "@/ipc/crop";
import { renditionFor } from "@/ipc/images";
import { DRAWER_HEIGHT, ZOOM_WHEEL_STEP } from "@/lib/constants";
import { cn } from "@/lib/utils";
import { useImageCrop } from "@/stores/crop";
import { useDrawerStore } from "@/stores/drawer";
import { usePreviewStore } from "@/stores/preview";
import { clampScale, type ImageTransform, useImageTransform, useTransformStore } from "@/stores/transform";
import { constrain, constrainPosition, fitInside, imageFraction, type Size } from "./geometry";
import { ProgressBar } from "./ProgressBar";

type PaneProps = {
    // Handed down rather than read from the store here, so that both panes draw the same photograph on
    // the same render and one preload serves the pair. See `hooks/useSettledFile.ts`.
    //
    // The dimensions alone rather than a whole `ImageRecord`, because the enhanced pane's result is
    // not one: it has no path, no format and no size on disk, and a record built by swapping the
    // result's identity and dimensions into the source's would carry the source's answers for
    // everything else - so anything this component read off it would be reading the wrong image's
    // data. Taking what is actually used means there is nothing to fabricate.
    /**
     * The photograph this pane is drawing - the settled one, not necessarily the current one.
     *
     * The **dimensions alone**, not the record they came off. Partial because a file whose header
     * could not be parsed publishes neither, and is still served and still drawn - the decoded
     * image's own size is the fallback.
     */
    published?: Partial<Size>;
    /** Where the pixels come from, or `undefined` for a file whose bytes could not be read. */
    source?: string;
    /** What the image is drawn as, for anything reading the page rather than looking at it. */
    alt: string;
    /** Which of the two this pane is, in the user's own language. */
    label: string;
    /** How this pane is placed: a flex half in two of the modes, the clipped overlay in the third. */
    className?: string;
    /** Which corner the chip sits in, and how loud it reads. */
    chipClassName: string;
    /** How far the chip rides above the pane's bottom edge, which the drawer's body pushes up. */
    chipBottom: number;
    // A style rather than a class: the clip and the divider's own `left` are one number, and a Tailwind
    // arbitrary value cannot carry a number that changes under the pointer.
    /** What the split comparison clips this pane to, and nothing else. */
    style?: CSSProperties;
    /**
     * Whether this pane is the one that reports what the canvas is showing to the sidebar.
     *
     * Exactly one pane sets it - the enhanced one, which is the only one drawn in all three
     * comparisons. See the store's `setViewport` for why there is no second writer to reconcile.
     */
    publishViewport?: boolean;
    // The enhanced pane is handed the **source's** identity while drawing the result's pixels, which is
    // the one thing that keeps the comparison a comparison: two panes at different zooms showing
    // different parts of a photograph is exactly what the second pane is not for. Every operation this
    // application runs preserves the aspect ratio, so both panes fit into the same box and resolve one
    // transform identically.
    //
    // Given rather than derived: it is the *source's* identity for both panes, while the enhanced
    // pane's pixels are addressed by the result's. Nothing about a URL says which of the two it is,
    // which is why this is stated rather than inferred.
    /** Which photograph's transform this pane reads and writes. Absent, the pane draws the fitted one. */
    transformKey?: string;
};

/** How far a pane's chip floats above whatever is beneath it, which is the design's own gap. */
const CHIP_GAP = 10;

/** Where a pane starts before it has measured itself, which in jsdom is also where it stays. */
const UNMEASURED: Size = { width: 0, height: 0 };

// The identity is carried with it because it is what makes a change of *image* different from a
// change of *scale* - see `resolve`.
//
// `identity: string | undefined` rather than the optional-property syntax the rest of this codebase
// prefers. Every read of it is an `===` against another `string | undefined`, where an absent key and
// an explicit `undefined` are indistinguishable - so the optional form buys nothing here and costs a
// conditional spread at the one place this is written.
//
// Requiring it is also what makes `resolve`'s return type honest: it answers a *position* and never
// an identity, so it returns `ImageTransform` and this is assembled from the two at the one call site
// that knows both.
/** What a pane has actually drawn: the resolved transform, and which photograph it was resolved for. */
type Drawn = { identity: string | undefined; scale: number; x: number; y: number };

/** How large the photograph is on screen at a given magnification. */
const scaled = (fitted: Size, scale: number): Size => ({ width: fitted.width * scale, height: fitted.height * scale });

/**
 * Where the photograph goes, given where it was and what has just been asked for.
 *
 * Two cases. At an unchanged scale the stored position is simply held inside the pane. At a changed
 * one, a single point of the photograph is pinned where it already is on screen - the point under
 * the pointer for a wheel zoom, and the middle of the pane for the drawer's slider and its two step
 * buttons, which are not pointed at any part of it.
 *
 * **A change of image counts as an unchanged scale**, whatever the two scales are.
 */
const resolve = (
    transform: ImageTransform,
    previous: Drawn,
    identity: string | undefined,
    fitted: Size,
    container: Size,
): ImageTransform => {
    const after = scaled(fitted, transform.scale);

    // The two cases are the reference's own split. A change of image joins the unchanged one:
    // re-anchoring against the position the *previous* photograph was drawn at would show an arbitrary
    // part of the new one, and returning to an image has to return to the part of it that was being
    // examined. The reference resolves both panes off one library instance and does not distinguish
    // the two.
    if (previous.scale === transform.scale || previous.identity !== identity) {
        const position = constrainPosition({ x: transform.x, y: transform.y }, after, container);

        return { scale: transform.scale, ...position };
    }

    const before = scaled(fitted, previous.scale);
    const anchor = {
        x: transform.anchor?.x ?? imageFraction(container.width / 2 - previous.x, before.width),
        y: transform.anchor?.y ?? imageFraction(container.height / 2 - previous.y, before.height),
    };

    // Where that point of the photograph is on screen right now, and where the position has to move
    // to so that it is still there once the photograph has changed size around it.
    const screen = { x: previous.x + anchor.x * before.width, y: previous.y + anchor.y * before.height };

    return {
        scale: transform.scale,
        x: constrain(screen.x - anchor.x * after.width, after.width, container.width),
        y: constrain(screen.y - anchor.y * after.height, after.height, container.height),
    };
};

/** One side of the comparison: the photograph, drawn at the size that fits and moved by a transform. */
const Pane = ({
    source,
    alt,
    label,
    className,
    chipClassName,
    chipBottom,
    style,
    publishViewport,
    transformKey,
    published,
}: PaneProps) => {
    const container = useRef<HTMLDivElement>(null);

    // What this pane's *view* is keyed on, which is not what its pixels are addressed by - see
    // `transformKey`. Everything below reads this: the stored transform, the "is this a different
    // photograph?" test in `resolve`, and the wheel handler's write.
    const identity = transformKey;
    const transform = useImageTransform(identity);
    const setTransform = useTransformStore((state) => state.setTransform);
    const setViewport = useTransformStore((state) => state.setViewport);

    const [measured, setMeasured] = useState<Size>(UNMEASURED);
    const [natural, setNatural] = useState<Size>(UNMEASURED);
    const [drawn, setDrawn] = useState<Drawn>({ identity: undefined, scale: transform.scale, x: 0, y: 0 });

    // The photograph's own dimensions: the published ones where the header could be read, and the
    // decoded image's otherwise - a file whose header this application could not parse is still served
    // and still drawn, and without the fallback it would have no size to be fitted into and would draw
    // as nothing.
    const imageWidth = published?.width ?? natural.width;
    const imageHeight = published?.height ?? natural.height;

    // Memoised because the resolution below is keyed on it: a fresh object per render would re-run
    // that effect on its own output, and re-resolving an anchored zoom against the position the
    // *store* still holds - which is the pre-zoom one - would undo the anchoring a frame later.
    const fitted = useMemo(
        () => fitInside(measured, { width: imageWidth, height: imageHeight }),
        [measured, imageWidth, imageHeight],
    );

    // Read by the two pointer handlers, which are registered once and must not close over a value
    // that changes sixty times a second. This is what the reference reads off its library instance.
    const live = useRef({ drawn, fitted, measured, identity });
    live.current = { drawn, fitted, measured, identity };

    // Measured on every resize, not once per image. That is the reference's own correction: measuring
    // once leaves the wheel anchor, the bounds and the published viewport all working off stale values
    // after a window resize or a sidebar toggle - and all three are wrong together, which is what makes
    // it hard to see.
    useResizeObserver(
        useCallback(() => container.current, []),
        useCallback((element: Element) => {
            const rect = element.getBoundingClientRect();

            // Same numbers means the same object, so a resize that changes nothing does not
            // re-render - and does not republish the viewport it would have recomputed.
            setMeasured((current) =>
                current.width === rect.width && current.height === rect.height
                    ? current
                    : { width: rect.width, height: rect.height },
            );
        }, []),
    );

    const requested = useRef<ImageTransform>(undefined);

    /*
     * Resolves what was asked for into what is drawn, and reports it to the sidebar.
     *
     * `drawn` is read through the ref rather than taken as a dependency: it is this effect's own
     * output, and depending on it would run the effect again on every resolution.
     */
    useEffect(() => {
        const previous = live.current.drawn;

        // A pane that has not been measured has resolved nothing: every position against a zero box
        // is zero, and the first real measurement has to resolve the stored transform rather than
        // re-constrain that. This is the state every pane mounts in, and the one every pane in
        // jsdom stays in without a stubbed rectangle.
        const ready = fitted.width > 0 && fitted.height > 0;

        // Nothing new was asked for, so this is a re-measurement - a window resize, a sidebar
        // toggle - and the photograph stays exactly where it is while the edges it is held between
        // are re-derived. Resolving the stored transform again instead would throw it back to the
        // position that transform was written at, which after an anchored zoom is the position it
        // was at *before* the zoom.
        const next =
            ready && transform === requested.current
                ? {
                      scale: previous.scale,
                      ...constrainPosition(previous, scaled(fitted, previous.scale), measured),
                  }
                : resolve(transform, previous, identity, fitted, measured);

        requested.current = ready ? transform : undefined;

        setDrawn((current) =>
            current.scale === next.scale &&
            current.x === next.x &&
            current.y === next.y &&
            current.identity === identity
                ? current
                : { ...next, identity },
        );

        if (!publishViewport) return;

        // The portion of the photograph inside this pane, as fractions of the whole of it. The
        // container is this pane's own box, so it is already half-width in side by side.
        const size = scaled(fitted, next.scale);
        if (size.width <= 0 || size.height <= 0) {
            setViewport({ x: 0, y: 0, width: 1, height: 1 });
            return;
        }

        const left = Math.max(0, next.x);
        const top = Math.max(0, next.y);
        const right = Math.min(measured.width, next.x + size.width);
        const bottom = Math.min(measured.height, next.y + size.height);

        setViewport({
            x: (left - next.x) / size.width,
            y: (top - next.y) / size.height,
            width: Math.max(0, right - left) / size.width,
            height: Math.max(0, bottom - top) / size.height,
        });
    }, [transform, fitted, measured, identity, publishViewport, setViewport]);

    /*
     * The wheel, and the trackpad pinch the webview delivers as one.
     *
     * A native, non-passive listener because React's synthetic `onWheel` is registered passive and
     * `preventDefault()` on it is a silent no-op. Per pane rather than once on the canvas, because the
     * handler needs this pane's own rectangle to turn a client coordinate into a point of the
     * photograph, and in side by side the pointer is over one particular half.
     *
     * **Every wheel event zooms, by a fixed step whatever its magnitude.** That is parity with the
     * reference's `ZOOM_WHEEL_STEP`, and it surprises: a two-finger scroll on a trackpad magnifies.
     */
    useEffect(() => {
        const element = container.current;
        if (!element) return;

        const onWheel = (event: WheelEvent) => {
            event.preventDefault();

            const { drawn: current, fitted: fittedNow, measured: measuredNow, identity: id } = live.current;
            if (id === undefined) return;

            // Wheel up is zoom in. Clamped here as well as in the store, because the position
            // resolved below is only the right position for a scale the store will actually keep:
            // resolving against 8.05 and then storing 8 would move the photograph by the difference.
            // The store's own `clampScale`, so the two cannot come to mean different numbers.
            const direction = event.deltaY < 0 ? 1 : -1;
            const scale = clampScale(current.scale + direction * ZOOM_WHEEL_STEP);

            const rect = element.getBoundingClientRect();
            const size = scaled(fittedNow, current.scale);
            const anchor = {
                x: imageFraction(event.clientX - rect.left - current.x, size.width),
                y: imageFraction(event.clientY - rect.top - current.y, size.height),
            };

            /*
             * Stored where the anchoring *lands*, rather than where the photograph is standing now.
             *
             * The pane resolves the anchor as it draws, so drawing is right either way - but the
             * store is what a photograph is handed back from when it becomes current again, and one
             * holding the pre-zoom position would return the user to a crop up to one notch's
             * anchoring correction away from the one they left. `anchor * fitted * ZOOM_WHEEL_STEP`
             * is 40px of an 800px pane at the edge of a photograph: small, visible, and a divergence
             * from "the same part of it as when they left it".
             *
             * Through `resolve` rather than by repeating its arithmetic, and off the same three
             * values the effect resolves against, so the position stored and the position drawn are
             * the same number computed once.
             *
             * The anchor rides along regardless: it is what a sibling pane applies to its own
             * container, which is the whole reason it is a fraction rather than a pixel offset - see
             * `imageFraction`.
             */
            const next = resolve({ scale, x: current.x, y: current.y, anchor }, current, id, fittedNow, measuredNow);

            setTransform(id, { scale: next.scale, x: next.x, y: next.y, anchor });
        };

        element.addEventListener("wheel", onWheel, { passive: false });

        return () => element.removeEventListener("wheel", onWheel);
    }, [setTransform]);

    /*
     * The drag that pans, coalesced to one store write per frame.
     *
     * Without the coalescing every pointer frame re-renders both panes, re-runs the resolution above
     * and republishes the viewport - "a sustained ~60Hz cascade through the tree for the length of a
     * drag", which is the reference's own measurement.
     */
    const grab = useRef<{ pointer: { x: number; y: number }; from: { x: number; y: number } }>(undefined);
    const pending = useRef<ImageTransform>(undefined);
    const frame = useRef<number>(undefined);
    const [dragging, setDragging] = useState(false);

    const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
        if (identity === undefined) return;

        // Capture rather than document-level listeners, because a drag that leaves the pane - and one
        // that leaves the window - must keep delivering moves to the element that started it, and the
        // capture is also what makes the release land in the right place.
        event.currentTarget.setPointerCapture(event.pointerId);
        grab.current = { pointer: { x: event.clientX, y: event.clientY }, from: { x: drawn.x, y: drawn.y } };
        setDragging(true);
    };

    const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
        const start = grab.current;
        const { drawn: current, fitted: fittedNow, measured: measuredNow, identity: id } = live.current;
        if (!start || id === undefined) return;

        const position = constrainPosition(
            {
                x: start.from.x + (event.clientX - start.pointer.x),
                y: start.from.y + (event.clientY - start.pointer.y),
            },
            scaled(fittedNow, current.scale),
            measuredNow,
        );

        pending.current = { scale: current.scale, ...position };
        if (frame.current !== undefined) return;

        frame.current = requestAnimationFrame(() => {
            frame.current = undefined;
            if (pending.current) setTransform(id, pending.current);
        });
    };

    const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId);
        }

        grab.current = undefined;
        setDragging(false);
    };

    /*
     * Flushed on unmount rather than cancelled, as the reference does: the coalesced write is the only
     * one that ever reaches the store, so dropping it loses up to a frame of panning. The identity
     * comes through the ref, so the flush lands on the image being dragged rather than on whichever
     * one this pane first mounted with.
     */
    useEffect(
        () => () => {
            if (frame.current === undefined) return;

            cancelAnimationFrame(frame.current);
            const id = live.current.identity;
            if (pending.current && id !== undefined) setTransform(id, pending.current);
        },
        [setTransform],
    );

    return (
        <div
            ref={container}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            style={style}
            className={cn(
                "relative min-w-0 flex-1 touch-none overflow-hidden",
                dragging ? "cursor-grabbing" : "cursor-grab",
                className,
            )}
            data-slot="preview-pane"
        >
            {/*
             * Spread conditionally, the shape `components/ui/slider.tsx` uses and for the same reason:
             * under `exactOptionalPropertyTypes` an explicit `undefined` is not an absent prop, and
             * `src=""` would resolve against the document and fetch the page itself.
             *
             * **No `object-contain`.** It centres the photograph by a rule the pane cannot read back,
             * and every number here - the bounds a pan stops at, the point a wheel zoom is anchored
             * on, the rectangle the sidebar draws - is derived from knowing exactly where the
             * photograph's top-left corner is. The `<img>` is drawn at its measured fitted size
             * instead, positioned at the pane's top-left, and moved with `translate(...) scale(...)`
             * about a `top left` origin.
             *
             * Sized from the measurement rather than by CSS, and left unsized until there is one - a
             * pane mounted hidden, or measured before layout, would otherwise draw a photograph of
             * zero width. `draggable` is off because the browser's own image drag would otherwise
             * start on top of the pan.
             */}
            <img
                {...(source !== undefined && { src: source })}
                alt={alt}
                draggable={false}
                onLoad={(event) =>
                    setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })
                }
                style={{
                    ...(fitted.width > 0 && { width: fitted.width, height: fitted.height }),
                    transform: `translate(${drawn.x}px, ${drawn.y}px) scale(${drawn.scale})`,
                    transformOrigin: "top left",
                }}
                className="block max-w-none select-none"
            />

            {/*
             * The offset is a style rather than a class pair, for the reason the drawer's own fold is:
             * the unfolded value is `DRAWER_HEIGHT` plus the design's 10px gap, and writing it as a
             * Tailwind step would be that sum restated in a second unit with nothing linking the two.
             */}
            <span
                data-slot="preview-chip"
                style={{ bottom: chipBottom }}
                className={cn(
                    "absolute rounded-full border border-border bg-background/80 px-2 py-0.5 text-[11px]",
                    chipClassName,
                )}
            >
                {label}
            </span>
        </div>
    );
};

/** Where the split's divider starts, and how far an arrow key moves it, as percentages of the canvas. */
const DIVIDER_START = 50;
const DIVIDER_STEP = 2;

/**
 * The current image, drawn the way the chosen comparison draws it.
 *
 * Three layouts over the same two panes: full draws the enhanced pane across the canvas, side by side
 * draws both in half of it each, and split draws the enhanced pane over the original and reveals it
 * from the divider rightward.
 *
 * **The enhanced pane draws whatever the current image's enhancements produced**, and the original
 * pane goes on drawing the source - which is what makes the two-pane and split comparisons show a
 * comparison. With no enhancements, with a run that has not landed yet, and after one that was
 * stopped, both panes point at the same URL, and both chips read *Original*.
 *
 * **The right-hand chip is what says a result landed.** It turns over to *Enhanced* exactly when the
 * pane stops drawing the source. A result is held while a later run works
 * (`hooks/useEnhancementRun.ts`), so re-running keeps the chip where it is instead of flickering back.
 *
 * **No bound on the URL**: the canvas asks for the photograph at its own size.
 *
 * **Both panes read one transform**, so magnifying or panning either moves both.
 *
 * **The canvas draws the settled image rather than the current one** (`hooks/useSettledFile.ts`).
 */
export const PreviewImage = () => {
    const { t } = useTranslation();
    // Everything a pane needs - the URL, the dimensions it is fitted from, the identity its transform
    // is stored under - comes off this one record, so a change of image reaches all of them on the
    // same render. Reading the current file here instead would put the new photograph's shape on the
    // old one's pixels for the length of a decode.
    const file = useSettledFile();
    const previewMode = usePreviewStore((state) => state.previewMode);

    /*
     * How the settled photograph is framed, which both panes draw through: the original pane asks the
     * protocol for the framing, and the enhanced pane draws a result that was *already* made from it,
     * so its own URL carries no crop. Against the settled file for the reason everything else on the
     * canvas is - a framing read off the current file would be applied to the outgoing photograph's
     * pixels for the length of a decode.
     */
    const crop = useImageCrop(file?.identity);
    /*
     * The run, owned here because this is the component that draws both panes and the bar, and is
     * mounted for as long as a file is open. Against the *settled* file, like everything else the
     * canvas keys off: a run started for a photograph that is not drawn yet would land a result on
     * the one still on screen.
     */
    const { enhanced, report, running, fraction } = useEnhancementRun(file, crop);

    /*
     * One URL for both panes and for all three modes, so switching modes re-points an `<img>` rather
     * than asking the protocol for anything new. Whether the *second pane* is free depends on the
     * platform: the response is served immutable, but as `ipc/images.ts` records, only WebView2 acts
     * on that - on macOS and Linux each `<img>` issues its own request and its own decode. Rust
     * answers the second from `Renditions` either way, so what repeats is the webview's decode, not
     * the encode. Bounding the panes the way the sidebar's miniature is bounded would change what the
     * canvas draws, so it is a decision for the canvas rather than a cleanup.
     *
     * No bound, which is `getImage(file, 0)` in the reference: the photograph at its own size is what
     * makes 8x a magnification of real pixels rather than of a canvas-sized rendition.
     *
     * Absent for a file whose pixels nothing can serve: the `<img>` then has no source and draws as
     * broken, which is also what a file deleted between being described and being drawn does.
     * Inventing a state for that here would be designing an error screen for a case no user has
     * reached.
     */
    const source = renditionFor(file, 0, crop);

    /*
     * The pane labels ride above whichever of the drawer's two states is showing, as the design's own
     * `chipBottom` does. The drawer slides over the canvas rather than resizing it, so this is the one
     * thing on the canvas that has to know it moved.
     */
    const drawerOpen = useDrawerStore((state) => state.open);
    const chipBottom = (drawerOpen ? DRAWER_HEIGHT : 0) + CHIP_GAP;

    /*
     * Where the split's divider sits, in this component's own state.
     *
     * Not in a store, because it is not a property of a photograph - switching images leaves it
     * alone, and nothing remounts this component to make that happen - and not a persisted
     * preference either: the reference does not keep it, and `stores/preview.ts` writes into the
     * Wails application's own key, which held exactly one field. Adding a second would be this
     * application writing something the application it replaces does not read.
     */
    const [divider, setDivider] = useState(DIVIDER_START);
    const handle = useRef<HTMLDivElement>(null);

    const moveDivider = (clientX: number) => {
        const rect = handle.current?.parentElement?.getBoundingClientRect();
        if (!rect || rect.width <= 0) return;

        // The same clamped fraction the panes take their anchor from, as a percentage - the width
        // guard above is what `imageFraction`'s own zero case would otherwise answer with a centred
        // divider rather than leaving this one where the user put it.
        setDivider(imageFraction(clientX - rect.left, rect.width) * 100);
    };

    // Drawn by `Preview`, which asks the same question to decide between this and the invitation.
    // Stated anyway, because a component that reads the current file has to say what it does without
    // one - and a file whose pixels cannot be served is the case above, not this one.
    if (!file) return;

    /*
     * The shape the photograph is drawn at, which is the framing's rather than the file's wherever one
     * is set. Both panes lay their box out from this *before* the pixels arrive, so it has to come
     * from the crop's own rectangle - see `framedDimensions`.
     */
    const framed = framedDimensions(file, crop);

    /*
     * What the enhanced pane draws: the result's own pixels and its own dimensions, or the source's
     * URL and the framing's shape while nothing has been produced - which is both panes pointing at
     * one URL, as the reference does for an empty stack. That costs no second encode - the identity
     * and the bound are the same, so Rust answers the second pane from `Renditions` - and on WebView2,
     * the one webview that acts on `immutable`, the webview's cache answers it outright. See the
     * comment above `source`.
     *
     * No framing on the result's own URL: the run was given the framed pixels, so what it produced is
     * already the enhancement of the framing and asking for it to be framed again would cut the
     * rectangle out of it a second time.
     */
    const enhancedSource = enhanced ? renditionFor(enhanced) : source;

    return (
        <div className="relative flex size-full flex-row gap-0.5 p-0.5">
            {/*
             * Absent in full, which draws the enhanced image alone - and present in split, where it is
             * the pane the other one is revealed over rather than a second half of the canvas.
             */}
            {previewMode !== "full" && (
                <Pane
                    published={framed}
                    {...(file.identity !== undefined && { transformKey: file.identity })}
                    {...(source !== undefined && { source })}
                    alt={t("common.previewAlt")}
                    label={t("preview.pane.original")}
                    chipClassName="left-2.5 text-foreground"
                    chipBottom={chipBottom}
                />
            )}

            <Pane
                published={enhanced ?? framed}
                {...(enhancedSource !== undefined && { source: enhancedSource })}
                {...(file.identity !== undefined && { transformKey: file.identity })}
                alt={t("common.previewAlt")}
                /*
                 * *Enhanced* only once a result has landed. This pane draws the source until
                 * then - with no enhancements at all, and for as long as the first run is still
                 * working - and a chip that called the source enhanced would be labelling the
                 * photograph by what was asked for rather than by what is on screen.
                 */
                label={enhanced ? t("preview.pane.enhanced") : t("preview.pane.original")}
                publishViewport
                /*
                 * `inset-0.5` is the container's own padding, so the overlay covers exactly the box
                 * the panes are laid out in. As a flex sibling it would be a second, half-width copy
                 * of the photograph instead of the same one superimposed.
                 *
                 * The clip and the divider's `left` below are one number rather than the matched pair
                 * of literals they were while the divider could not move.
                 */
                className={cn(previewMode === "split" && "absolute inset-0.5")}
                {...(previewMode === "split" && { style: { clipPath: `inset(0 0 0 ${divider}%)` } })}
                chipClassName="right-2.5 text-foreground"
                chipBottom={chipBottom}
            />

            {running && <ProgressBar fraction={fraction} {...(report && { report })} />}

            {previewMode === "split" && (
                /*
                 * A real `role="slider"` with arrow keys, rather than a `<div>` with a pointer
                 * handler: the reference's handle is reachable by no route but the mouse, and this
                 * codebase has taken the other answer at every turn. A keydown handler moving the
                 * divider by 2% is what makes the third comparison operable.
                 *
                 * **The divider is the only draggable thing**, which is the reference's
                 * `onlyHandleDraggable` - and here it is not a preference: the panes themselves are
                 * the pan surface, so a drag started on a pane and a drag started on the divider have
                 * to mean different things.
                 */
                <div
                    ref={handle}
                    role="slider"
                    tabIndex={0}
                    aria-label={t("preview.divider")}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={Math.round(divider)}
                    style={{ left: `${divider}%` }}
                    onPointerDown={(event) => {
                        event.currentTarget.setPointerCapture(event.pointerId);
                        moveDivider(event.clientX);
                    }}
                    onPointerMove={(event) => {
                        if (event.currentTarget.hasPointerCapture(event.pointerId)) moveDivider(event.clientX);
                    }}
                    onPointerUp={(event) => {
                        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                            event.currentTarget.releasePointerCapture(event.pointerId);
                        }
                    }}
                    onKeyDown={(event) => {
                        if (event.key === "ArrowLeft") setDivider((at) => Math.max(at - DIVIDER_STEP, 0));
                        else if (event.key === "ArrowRight") setDivider((at) => Math.min(at + DIVIDER_STEP, 100));
                        else return;

                        event.preventDefault();
                    }}
                    className="absolute top-0 bottom-0 z-[3] w-0.5 cursor-ew-resize touch-none bg-foreground outline-hidden"
                    data-slot="preview-divider"
                >
                    <span className="absolute top-1/2 left-1/2 flex size-8 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border-2 border-foreground bg-background/75">
                        <ChevronsLeftRight className="size-4" />
                    </span>
                </div>
            )}
        </div>
    );
};
