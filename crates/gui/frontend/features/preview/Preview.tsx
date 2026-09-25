import { useRef } from "react";
import { ShineBorder } from "@/components/ui/shine-border";
import { PreviewEmpty } from "@/features/preview/PreviewEmpty";
import { PreviewImage } from "@/features/preview/PreviewImage";
import { useDroppedImages } from "@/hooks/useDroppedImages";
import { useCanvasSurface } from "@/lib/canvas";
import { DRAWER_HEIGHT } from "@/lib/constants";
import { cn } from "@/lib/utils";
import { useDrawerStore } from "@/stores/drawer";
import { useCurrentFile } from "@/stores/files";

/** How long one sweep of the shine around the canvas's border takes, in seconds. */
const SHINE_DURATION = 5;

/** How thick the shine is, in pixels. */
const SHINE_WIDTH = 2;

/**
 * The two colours the shine runs through, which are the pair the design gives the Autopilot
 * analysing spinner.
 *
 * A list rather than a single colour: `ShineBorder` drops them into one radial gradient in the
 * order given, so the two arrive together and sweep the border as one mark - where the beams this
 * replaced carried one colour each and read as a set only because they were evenly spaced.
 */
const SHINE_COLORS = ["var(--success-bright)", "var(--activity)"];

/**
 * The canvas the image is shown on: the invitation to supply one until there is one, and the current
 * image after that.
 *
 * Absolutely positioned with a 48px bottom inset, and the drawer is drawn *over* it rather than
 * below it - so the canvas is permanently 48px short of the window's bottom edge whether the drawer
 * is folded or not. That is what makes folding cheap: nothing re-lays-out, and the preview image
 * never rescales. Both values are the Wails app's and what the design draws.
 *
 * The question is asked here rather than inside either child, and it is the same one the navbar and
 * the sidebar ask: `useCurrentFile` is the one accessor for "which photograph", so four regions
 * cannot come to disagree about whether there is one.
 *
 * The drop route in is listened to here, and hit-tested against this element, because this is the
 * region that offers it: the empty canvas is what says images can be dragged and dropped, and the
 * rectangle that says so is the rectangle that accepts one. Tauri delivers a drop as a window event
 * rather than to an element, so the ref is how the hook learns which rectangle that is.
 *
 * **What the same hook reports while a drag is still in the air is what lights the region below.**
 * One listener and one hit-test answer both questions, which is what keeps the rectangle that is
 * drawn and the rectangle that accepts a drop from ever becoming two different rectangles.
 *
 * **What is behind both states is one question, asked once.** `useCanvasSurface` answers it for the
 * crop dialog too, which is what keeps the two regions drawing the same thing without either of them
 * knowing about the other - see `lib/canvas.tsx`.
 */
export const Preview = () => {
    const file = useCurrentFile();
    const region = useRef<HTMLDivElement>(null);
    const { className, field } = useCanvasSurface();

    // How much of the canvas the drawer is currently standing on. The canvas is inset by the folded
    // header permanently, so this is the body alone - what unfolding adds.
    const covered = useDrawerStore((state) => state.open) ? DRAWER_HEIGHT : 0;

    const dragging = useDroppedImages(region);

    return (
        <div
            data-slot="preview-canvas"
            className={cn("absolute inset-x-0 top-0 bottom-12 flex items-center justify-center", className)}
        >
            {/*
             * Drawn first and lifted under, as the design lays it out: the field is a sibling of the
             * canvas's content rather than a layer behind the element, so what the canvas has to say
             * - the invitation, or the photograph - is over it in both states. `relative z-10` is
             * what does the lifting, since the field itself is `absolute inset-0`.
             */}
            {field}

            <div className="relative z-10 flex h-full w-full items-center justify-center">
                {file ? <PreviewImage /> : <PreviewEmpty />}
            </div>

            {/*
             * Where a dragged file will be taken: the rectangle the drop is hit-tested against, and
             * the rectangle the shine is drawn on while a drag is over it. **It is one element for
             * both**, which is how the spec's promise - that what is offered and what is taken up
             * are the same rectangle - stops being something two pieces of code have to agree about.
             * It is always mounted, because it is the hit test; only the shine comes and goes.
             *
             * Drawn in both of the canvas's states, over the photograph as well as over the
             * invitation, because a drop is accepted in both.
             *
             * **Its bottom edge is the drawer's top, not the canvas's.** The canvas is inset by the
             * folded drawer's 48px header permanently and unfolding slides the 128px body up *over*
             * it rather than resizing anything, so the canvas's own bottom edge sits in the middle
             * of an open drawer. A region that used it would draw its bottom edge across the strip
             * of thumbnails and would accept a drop let go of on them - which is the one thing
             * `useDroppedImages` said a later slice would have to answer. This is that answer: the
             * offered region is the part of the canvas the drawer is not standing on.
             *
             * **On the canvas's own border.** An earlier revision inset it by 16px, on the reasoning
             * that this element's left, right and top edges are the window's own and a lit line
             * along them would read as window chrome. Seen in the window it read as something worse
             * - a second rectangle floating inside the canvas, which is not the region that accepts
             * anything. The mark traces the region, so it traces its edges. See design.md D5.
             *
             * **`rounded-xl`**, the 12px radius the design gives everything else the application
             * draws over this canvas, and the shine takes it exactly: it inherits the radius and is
             * masked to the ring that radius describes. The beams this replaced could not - a rigid
             * square travelling an `offset-path` had to be given a corner scaled to its own length
             * or it stalled on one - so the rectangle the user sees is now the rectangle drawn.
             *
             * **No track under the shine.** A `--border` line was drawn here first, on the design's
             * analysing spinner - whose two colours these are, and whose arcs do ride a visible
             * ring. On a rectangle this size it read as a grey box being drawn around the canvas
             * rather than as the ground under a light, so only the light is left. Where the beams
             * needed to be long to carry the shape between them, the shine is the whole border at
             * once: the gradient is over every edge, and what sweeps is which part of it is lit.
             *
             * `pointer-events-none` is load-bearing rather than tidy. This is a full-bleed element
             * over a canvas that carries the drop itself and the wheel-zoom and drag-to-pan
             * handlers, and one that intercepted a pointer would break the very drop it advertises.
             */}
            <div
                ref={region}
                data-slot="preview-drop-region"
                aria-hidden="true"
                className="pointer-events-none absolute inset-x-0 top-0 z-20 rounded-xl"
                style={{ bottom: covered }}
            >
                {dragging && (
                    <ShineBorder borderWidth={SHINE_WIDTH} duration={SHINE_DURATION} shineColor={SHINE_COLORS} />
                )}
            </div>
        </div>
    );
};
