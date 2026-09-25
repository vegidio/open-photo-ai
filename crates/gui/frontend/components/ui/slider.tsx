"use client";

import { type ComponentProps, useCallback, useEffect, useMemo, useState } from "react";
import { Slider as SliderPrimitive } from "radix-ui";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

const Slider = ({
    className,
    defaultValue,
    value,
    min = 0,
    max = 100,
    thumbLabel,
    formatValue,
    track = true,
    ...props
}: ComponentProps<typeof SliderPrimitive.Root> & {
    /**
     * The accessible name of the thumb - added to what `shadcn add` generates.
     *
     * Radix puts `role="slider"` on the **thumb**, not on the root, so an `aria-label` passed to this
     * component lands on an element that has no role to name. Without this, a screen has no way to
     * tell one slider from another: the settings dialog draws four, one per export format, and all
     * four would announce as an unnamed slider carrying a number.
     */
    thumbLabel?: string;
    /**
     * How a thumb's value reads in the bubble above it - also added to what `shadcn add` generates.
     *
     * Opt-in rather than always on, because a slider that already prints its value beside itself does
     * not want a second copy of it floating over the dialog: the four export rows carry a `w-7.5`
     * value span, and the drawer's zoom carries nothing, which is what makes it the one that needs
     * this. Passing the formatter rather than a boolean keeps the unit ("4x", "60%") with the control
     * that knows it, and keeps this component out of the business of translating anything.
     */
    formatValue?: (value: number) => string;
    /**
     * Whether the track is filled from the low end up to the thumb - the third addition to what
     * `shadcn add` generates, and the only one that is on by default.
     *
     * That fill says "this much of a quantity", which is true of every slider in this application
     * but one. The crop dialog's fine rotation runs from -90 to +90 about a meaningful zero, so the
     * fill paints the whole left half of the track solid at 0 degrees and reads as a value set to
     * something rather than as a photograph that is not turned at all. Off, the control is a track
     * with a thumb on it, which is what the design draws and what the reference gets from MUI's own
     * `track={false}`.
     *
     * Named for the prop the reference passes rather than for the element Radix calls `Range`, since
     * what a caller is deciding is whether the slider *has* a filled track.
     */
    track?: boolean;
}) => {
    const _values = useMemo(
        () => (Array.isArray(value) ? value : Array.isArray(defaultValue) ? defaultValue : [min, max]),
        [value, defaultValue, min, max],
    );

    /*
     * Four pieces of state rather than Radix's own tooltip behaviour, because that behaviour is wrong
     * for a thumb you drag. `TooltipTrigger` closes on `pointerdown` and refuses to reopen until the
     * pointer has left and come back, so the bubble would vanish for exactly the gesture it exists to
     * narrate - and the pointer slips off a 16px thumb constantly during a drag, which would flicker
     * it even if the close were removed.
     *
     * So `open` is driven from here instead: the pointer resting on a thumb, a drag in progress, or a
     * key pressed since the thumb took focus. `dragging` and `adjusting` are about the slider as a
     * whole, so they are paired with `active` - the focused thumb, which is the one Radix moves - to
     * name which bubble they open. A single-thumb slider gets the same answer either way; a range
     * slider would otherwise light up both of its thumbs from one drag.
     */
    const [hovered, setHovered] = useState<number>();
    const [active, setActive] = useState<number>();
    const [dragging, setDragging] = useState(false);
    const [adjusting, setAdjusting] = useState(false);

    const endDrag = useCallback(() => setDragging(false), []);

    // A `once` listener on the document rather than `onPointerUp` on the root: a drag that started on
    // the thumb ends wherever the pointer happens to be, which is routinely outside this component.
    // The cleanup is for the drag that is interrupted by an unmount - `once` never fires then.
    useEffect(() => () => document.removeEventListener("pointerup", endDrag), [endDrag]);

    const thumb = (index: number) => (
        <SliderPrimitive.Thumb
            data-slot="slider-thumb"
            // Thumbs are positional - thumb 0 is always the lower bound of a range - and the list is
            // generated from a count, not from data that can reorder. There is no other key
            // available, and the rule's failure mode (state following the wrong item after a
            // reorder) cannot occur here. Unsuppressed because biome only sees the index key on the
            // `map` below, where the tooltip branch repeats it.
            key={index}
            {...(thumbLabel !== undefined && { "aria-label": thumbLabel })}
            onPointerEnter={() => setHovered(index)}
            // Guarded rather than a bare `setHovered(undefined)`: the pointer can enter the next thumb
            // of a range slider before it leaves this one, and an unguarded clear would then wipe the
            // index that enter just wrote.
            onPointerLeave={() => setHovered((current) => (current === index ? undefined : current))}
            onFocus={() => setActive(index)}
            onBlur={() => {
                setActive((current) => (current === index ? undefined : current));
                setAdjusting(false);
            }}
            className="block size-4 shrink-0 rounded-full border border-primary bg-white shadow-sm ring-ring/50 transition-[color,box-shadow] hover:ring-4 focus-visible:ring-4 focus-visible:outline-hidden disabled:pointer-events-none disabled:opacity-50"
        />
    );

    return (
        <SliderPrimitive.Root
            data-slot="slider"
            // Spread conditionally rather than passed straight through. Both props are destructured
            // above so the thumb count can be derived from them, which makes each `T[] | undefined`
            // here - and under `exactOptionalPropertyTypes` an explicit `undefined` is no longer the
            // same thing as an absent prop, so forwarding it would flip Radix's own
            // controlled/uncontrolled detection. Omitting the key keeps that detection intact.
            {...(defaultValue !== undefined && { defaultValue })}
            {...(value !== undefined && { value })}
            min={min}
            max={max}
            className={cn(
                "relative flex w-full touch-none items-center select-none data-disabled:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:flex-col",
                className,
            )}
            {...props}
            /*
             * After the spread and calling through by hand, so a caller keeps its own handlers: these
             * two are this component's, not a replacement for the consumer's.
             *
             * On the root rather than on the thumb because a press on the *track* is a drag too -
             * Radix jumps the nearest thumb to the press and then follows the pointer - and because
             * a key only ever reaches the thumb, which is the one focusable thing in here.
             */
            onPointerDown={(event) => {
                props.onPointerDown?.(event);
                setAdjusting(false);
                setDragging(true);
                document.addEventListener("pointerup", endDrag, { once: true });
            }}
            onKeyDown={(event) => {
                props.onKeyDown?.(event);
                setAdjusting(true);
            }}
        >
            <SliderPrimitive.Track
                data-slot="slider-track"
                className={cn(
                    "relative grow overflow-hidden rounded-full bg-muted data-[orientation=horizontal]:h-1.5 data-[orientation=horizontal]:w-full",
                )}
            >
                {track && (
                    <SliderPrimitive.Range
                        data-slot="slider-range"
                        className={cn(
                            "absolute bg-primary data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full",
                        )}
                    />
                )}
            </SliderPrimitive.Track>
            {_values.map((thumbValue, index) =>
                formatValue === undefined ? (
                    thumb(index)
                ) : (
                    /*
                     * Controlled, and never open while the control is unavailable: a disabled slider
                     * still receives pointer events - `disabled:` in the thumb's classes is the CSS
                     * pseudo-class, and Radix renders the thumb as a `<span>`, which never matches it
                     * - so without this a greyed-out zoom would still narrate 1x under the pointer.
                     */
                    <Tooltip
                        // biome-ignore lint/suspicious/noArrayIndexKey: the index IS this list's identity
                        key={index}
                        open={
                            props.disabled !== true &&
                            (hovered === index || ((dragging || adjusting) && active === index))
                        }
                    >
                        <TooltipTrigger asChild>{thumb(index)}</TooltipTrigger>

                        {/*
                         * Offset off the thumb rather than the app's usual flush tooltip: this trigger
                         * grows a 4px ring on hover, which is exactly when the bubble is showing, and
                         * at the default 0 the arrow would sit inside it.
                         */}
                        <TooltipContent sideOffset={6}>{formatValue(thumbValue)}</TooltipContent>
                    </Tooltip>
                ),
            )}
        </SliderPrimitive.Root>
    );
};

export { Slider };
