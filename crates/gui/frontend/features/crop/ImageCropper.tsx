import type { ComponentProps, Ref } from "react";
import { Cropper, CropperBackgroundImage, type CropperRef, type CropperState } from "react-advanced-cropper";
import { CROP_ZOOM_WHEEL_RATIO, MIN_CROP_SIZE } from "@/lib/constants";
import { cn } from "@/lib/utils";

/** The box a photograph opens with when nothing is recorded for it: the whole of it. */
const defaultSize = ({ imageSize }: CropperState) => ({ width: imageSize.width, height: imageSize.height });

/**
 * The photograph, clipped to the rectangle the crop box can actually be dragged out to.
 *
 * **This is what stops a magnified photograph showing pixels that cannot be cropped.**
 */
const ClippedBackground = ({ ref, ...props }: ComponentProps<typeof CropperBackgroundImage>) => (
    // The widget nests like this:
    //
    // ```
    // .advanced-cropper            the element below, carrying `p-1.5` - and `overflow: hidden`
    //   .advanced-cropper__boundary    an ordinary flex child, so 6px smaller on every side
    //     .__background-wrapper        inset 0 of the boundary
    //       <img>                      this component
    //       .rectangle-stencil         the box, confined to the boundary; its handles overhang it
    // ```
    //
    // The 6px ring the padding opens up is what the drag handles need - each is a 10px square centred
    // on the box's edge, so five of those pixels fall outside the box and would be sliced off at the
    // photograph's own edge without it. But the stencil is confined to the *boundary*, and without this
    // element the image is clipped only at the *root*, 6px further out. Fitted that is invisible,
    // because the photograph is inside the boundary anyway. Magnified it is not: the image fills the
    // ring as well, and those six pixels a side are photograph that the box can never be dragged onto.
    //
    // The two cannot share a clip - one has to stop at the boundary and the other has to escape it -
    // and the image and the stencil are siblings with no element between them. So the image is given
    // a clipping parent of its own here, matching the boundary exactly, and the handles are still
    // clipped by the root. Magnified then behaves as fitted does: the photograph stops where the box
    // stops, and the ring holds nothing but the surface behind.
    //
    // `inset-0` against `__background-wrapper`, which is itself `inset-0` of the boundary, so this
    // element is the boundary's rectangle to the pixel - the `<img>` keeps the containing block it had
    // and is positioned by exactly the styles the widget already computed for it.
    //
    // `pointer-events-none` so hit testing is what it was: the wrapper above handles the drag and
    // the wheel, and this element is not a new target between it and them.
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <CropperBackgroundImage ref={ref} {...props} />
    </div>
);

/**
 * The cropper widget, with this application's answers to the handful of questions it asks.
 *
 * It fills the pane it is given, leaves the surround outside the rectangle undimmed, and draws the
 * thirds grid permanently. `ref` is the widget's whole imperative surface, and the controller holds
 * it.
 */
export const ImageCropper = ({
    ref,
    src,
    aspectRatio,
    onChange,
    onReady,
    className,
}: {
    ref?: Ref<CropperRef>;
    src: string;
    aspectRatio?: number;
    /*
     * Both required rather than optional, which `exactOptionalPropertyTypes` makes the simpler
     * shape as well as the honest one: the widget's own props do not admit `undefined`, so an
     * optional callback here would have to be spread conditionally at every forward. The dialog is
     * the only caller and passes both - a cropper nothing listens to is not a thing this app wants.
     */
    onChange: (cropper: CropperRef) => void;
    onReady: (cropper: CropperRef) => void;
    className?: string;
}) => (
    // A wrapper rather than a `<Cropper>` at the call site, for the reason every other wrapper in
    // `components/ui/` exists: the grid, the surround, the minimum size and the wheel ratio are
    // decisions about what framing *is* here, not about the dialog's layout, and the dialog should not
    // be the thing that remembers them. React 19 passes `ref` through as an ordinary prop, so there is
    // no `forwardRef`.
    //
    // **The geometry and the stencil styling are the reference's exactly.** Each of the four classes
    // below - `size-full`, `p-1.5`, `text-transparent` on the overlay and `opacity-100` on the grid -
    // looks arbitrary and each is load-bearing. None carries the `!` suffix the reference needs,
    // because `frontend/style.css` imports the widget's stylesheet below Tailwind's utility layer (and
    // says why) - so these four classes are exactly the places to look if that ever stops holding.
    <Cropper
        ref={ref}
        src={src}
        defaultSize={defaultSize}
        /*
         * **Both off, and both are load-bearing rather than tuning.**
         *
         * `crossOrigin` defaults to `true` in this widget, which sets `crossOrigin="anonymous"` on
         * the image it loads whenever the source is cross-origin - and `opai://localhost/...` is
         * cross-origin to the page whether that page is the dev server or the bundled app. That
         * turns an ordinary image load into a CORS request, and `crates/gui/src/images/serve.rs`
         * sends no `Access-Control-Allow-Origin`: the load fails and the dialog draws no photograph
         * at all. The canvas has always drawn these renditions from a plain `<img>` with no
         * `crossOrigin`, and this is how the dialog asks the same way.
         *
         * `checkOrientation` defaults to `true` and fetches the whole image a second time over XHR
         * purely to read an EXIF tag. It is wasted on a rendition - Rust decoded, oriented and
         * re-encoded these pixels, so there is no orientation left to recover - and on a 3072px
         * rendition it is a second full transfer per opening. Off, the widget sees exactly what the
         * canvas sees, which is the property the framing has to be chosen against.
         *
         * The reference sets neither, because it hands the widget a `blob:` URL from its own Wails
         * IPC rather than a custom-scheme URL - same-origin, and already stripped of its metadata.
         */
        crossOrigin={false}
        checkOrientation={false}
        // The photograph clipped to the boundary rather than to the pane - see the component.
        backgroundComponent={ClippedBackground}
        /*
         * A *ratio* of the current scale per wheel event, which is the widget's own knob and is not
         * the canvas's `ZOOM_WHEEL_STEP` - see `lib/constants.ts` for why the two are separate
         * numbers. Pinch arrives through the same path, since a trackpad pinch is a ctrl-wheel.
         */
        backgroundWrapperProps={{ scaleImage: { wheel: { ratio: CROP_ZOOM_WHEEL_RATIO } } }}
        onChange={onChange}
        onReady={onReady}
        stencilProps={{
            grid: true,
            // Spread rather than passed, because an explicit `undefined` is not an absent prop under
            // `exactOptionalPropertyTypes` - and absent is what "no constraint" has to be here.
            ...(aspectRatio !== undefined && { aspectRatio }),
            // In the widget's own space; `MIN_CROP_SIZE`'s docs say why that is the conservative reading.
            minWidth: MIN_CROP_SIZE,
            minHeight: MIN_CROP_SIZE,
            // **The surround is not dimmed at all.** The widget draws it as `box-shadow: 0 0 0 1000px
            // currentColor`, which spreads to the nearest clipping ancestor - so any visible colour here
            // washes the whole pane rather than the photograph, and the dialog reads as a black
            // semi-transparent panel laid over the dotted surface. What is kept is told from what is
            // not by the rectangle's own outline and its thirds grid, which is what the reference does
            // and what `gui-crop` records.
            overlayClassName: "text-transparent",
            // The widget fades the thirds grid in only during an interaction, and both the design and
            // `gui-crop` draw it permanently.
            gridClassName: "opacity-100",
        }}
        // `size-full`: the widget fills the pane it is given. Anything that shapes this box to the
        // photograph instead draws the photograph smaller than the space reserved for it.
        //
        // `p-1.5`: the 6px the drag handles need. They are drawn centred on the rectangle's edge, so at
        // the photograph's own edge they overhang it; without this ring, and under any clip tighter
        // than this element, they come out visibly cut. It is paired with `ClippedBackground`, which is
        // what keeps the ring from filling with photograph the box cannot reach once it is magnified -
        // read that first, because the padding on its own is only half of the arrangement.
        className={cn("size-full bg-transparent object-contain p-1.5", className)}
    />
);
