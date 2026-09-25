import { type ChangeEvent, useEffect, useState } from "react";
import { Slider } from "@/components/ui/slider";
import { clampTo } from "@/lib/utils";

/** How far one step of the slider moves, in percent: the reference's `IntensitySelector` value. */
const STEP = 5;

/**
 * Half the width of the slider's thumb, in pixels - `size-4` in `components/ui/slider.tsx`.
 *
 * Radix keeps the thumb inside the track by shifting it towards the centre by up to this much, so a
 * mark placed at the bare percentage sits under the thumb only at 50%. See {@link markLeft}.
 */
const THUMB_HALF = 8;

type IntensityControlProps = {
    /** What the control is called, in the words the caller's family uses for its parameter. */
    label: string;
    /** The value the enhancement carries, as a whole percentage. */
    value: number;
    /** The smallest and largest the selected model publishes, in percent; absent until the catalogue has arrived. */
    min?: number;
    max?: number;
    /** The value labelled beneath the track - the parameter's neutral one. */
    mark: number;
    /** Called with a whole percentage, already inside the range where there is one. */
    onChange: (value: number) => void;
};

/**
 * Where the mark for `value` has to sit to be under the thumb when the thumb is at `value`.
 *
 * Radix's own `getThumbInBoundsOffset`, restated: the thumb is shifted right by its half-width at the
 * low end, not at all at the centre, and left by its half-width at the high end.
 */
const markLeft = (value: number, min: number, max: number) => {
    const percent = ((value - min) / (max - min)) * 100;

    return `calc(${percent}% + ${THUMB_HALF * (1 - percent / 50)}px)`;
};

/**
 * An amount as a whole percentage: a typed field beside a slider over the same value, with the
 * parameter's neutral value marked beneath the track.
 *
 * **It knows nothing about any one family.** The label, the range and the mark are the caller's, and
 * the unit it speaks is percent - so light adjustment's -100..100 about 0 and a later denoise's
 * 0..300 about 100 are the same control handed different numbers. See design.md D2.
 *
 * **The slider writes when it is released**, not at each step of a drag: every write re-runs the
 * preview, and a run superseded mid-flight is a model call thrown away. The position it is dragged
 * through is held here and shown in the field, which is what the reference's `ValueSlider` does with
 * `onChangeCommitted`.
 *
 * **An empty field and a bare minus sign are held**, not written: both are keystrokes on the way to a
 * number, and the second is the only way to reach a negative one. Leaving the field restores what the
 * enhancement carries. Anything that parses as an integer is brought inside the range and written.
 *
 * With no bounds - the render before the catalogue arrives - nothing is clamped and the slider has no
 * range to draw a thumb on, so it is drawn empty and unavailable rather than over a range invented here.
 */
export const IntensityControl = ({ label, value, min, max, mark, onChange }: IntensityControlProps) => {
    /*
     * What the field shows when no drag is in progress, which is not always what the enhancement
     * carries: `` and `-` are states of the input no percentage corresponds to. Re-seeded whenever the
     * enhancement changes under it.
     */
    const [text, setText] = useState(String(value));

    /* Where the thumb is while it is being dragged, and nothing once it has been released. */
    const [dragged, setDragged] = useState<number>();

    useEffect(() => {
        setText(String(value));
        setDragged(undefined);
    }, [value]);

    const bounded = min !== undefined && max !== undefined;

    /** Into the published range, where there is one. */
    const clamp = (percent: number) => clampTo(percent, min, max);

    /* Writing the stack is what asks for a new run, so a value that did not move writes nothing. */
    const write = (percent: number) => {
        setText(String(percent));
        if (percent !== value) onChange(percent);
    };

    const onType = (event: ChangeEvent<HTMLInputElement>) => {
        const typed = event.target.value.trim();

        if (typed === "" || typed === "-") {
            setText(typed);
            return;
        }

        const percent = Number.parseInt(typed, 10);
        if (Number.isNaN(percent)) return;

        // Brought inside the range rather than refused, which the seam does again on the far side.
        write(clamp(percent));
    };

    return (
        <div className="flex flex-col gap-5">
            <div className="flex items-center justify-between gap-2">
                <span className="text-[13px] text-foreground">{label}</span>

                {/*
                 * `inputMode` rather than `type="number"`, for `ScaleControl`'s reason: a number
                 * input's own validation would swallow the bare `-` this control exists to hold.
                 */}
                <label className="flex h-8 flex-none basis-1/3 items-center justify-between rounded-md border border-input bg-background px-2">
                    <input
                        inputMode="numeric"
                        value={dragged === undefined ? text : String(dragged)}
                        onChange={onType}
                        onBlur={() => setText(String(value))}
                        aria-label={label}
                        className="w-full min-w-0 bg-transparent font-mono text-[13px] outline-hidden"
                    />
                    <span className="text-[13px] text-muted-foreground">%</span>
                </label>
            </div>

            <div className="relative mx-1 mb-[23px]">
                <Slider
                    {...(bounded && { min, max })}
                    step={STEP}
                    // No thumb without a range: there is nowhere honest to put one.
                    value={bounded ? [dragged ?? value] : []}
                    disabled={!bounded}
                    thumbLabel={label}
                    // A bare track about a neutral value, as the crop dialog's rotation draws it. The
                    // design paints this track a tier lighter than the shared slider's default.
                    track={false}
                    className="**:data-[slot=slider-track]:bg-input"
                    onValueChange={([next]) => next !== undefined && setDragged(next)}
                    onValueCommit={([next]) => {
                        setDragged(undefined);
                        if (next !== undefined) write(next);
                    }}
                />

                {bounded && (
                    <span
                        aria-hidden
                        style={{ left: markLeft(mark, min, max) }}
                        className="pointer-events-none absolute top-[calc(100%+8px)] -translate-x-1/2 text-[10px] text-foreground-faint"
                    >
                        {mark}
                    </span>
                )}
            </div>
        </div>
    );
};
