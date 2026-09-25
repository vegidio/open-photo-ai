import type { ReactNode } from "react";
import { FlipHorizontal, FlipVertical, RotateCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";

/** The fine rotation's range, in degrees. A quarter turn either way; past that is the quarter-turn action. */
const FINE_MIN = -90;
const FINE_MAX = 90;

/** How far apart the slider's marks are, which is the design's own 15-degree spacing. */
const MARK_STEP = 15;

// Positions rather than a `<datalist>`: Radix's slider draws no marks of its own, so they are
// absolutely positioned against the same track the thumb runs on. Derived from the range and the
// step rather than written out, so the three numbers above are the only place the scale is stated.
/** The marks, as fractions of the track, both ends included. */
const MARKS = Array.from(
    { length: (FINE_MAX - FINE_MIN) / MARK_STEP + 1 },
    (_, index) => ((index * MARK_STEP) / (FINE_MAX - FINE_MIN)) * 100,
);

/**
 * The row under the photograph: the quarter turn, the two mirrors, the fine rotation and Reset.
 *
 * Screen 18 draws it centred under the cropper pane, three 32px icon buttons then the slider then a
 * 112px Reset.
 *
 * **Every icon-only control here carries an accessible name.**
 */
export const RotateControls = ({
    rotation,
    onRotationChange,
    onRotate90,
    onFlipHorizontal,
    onFlipVertical,
    onReset,
    className,
}: {
    rotation: number;
    onRotationChange: (value: number) => void;
    onRotate90: () => void;
    onFlipHorizontal: () => void;
    onFlipVertical: () => void;
    onReset: () => void;
    className?: string;
}) => {
    const { t } = useTranslation();

    // Named, which the reference's are not: its `IconButton`s pass no label at all, so a screen reader
    // announces three unnamed buttons in a row and the user has to operate one to find out what it
    // was. That is an omission rather than a decision, and this application has named every icon-only
    // control it has shipped.
    const action = (label: string, icon: ReactNode, onClick: () => void) => (
        <Button
            type="button"
            variant="outline"
            size="icon-sm"
            aria-label={label}
            onClick={onClick}
            className="flex-none border-input bg-transparent dark:bg-transparent [&_svg:not([class*='size-'])]:size-4.5"
        >
            {icon}
        </Button>
    );

    return (
        <div
            data-slot="rotate-controls"
            className={cn("flex w-full max-w-180 items-center gap-4 self-center px-4 py-2", className)}
        >
            {action(t("crop.rotate"), <RotateCw />, onRotate90)}
            {action(t("crop.flipHorizontal"), <FlipHorizontal />, onFlipHorizontal)}
            {action(t("crop.flipVertical"), <FlipVertical />, onFlipVertical)}

            {/*
             * The slider sits directly in the row rather than in a column of its own, which is what
             * puts it on the same centreline as the buttons either side of it.
             */}
            <div className="relative flex min-w-50 flex-1 items-center">
                {/*
                 * No `thumbLabel`, which is a scope decision rather than an oversight: the dialog's five
                 * icon-only controls are labelled, and the slider is not one of them - it carries its
                 * marked scale and a degree bubble on the thumb. A sixth catalogue key across thirteen
                 * locales belongs to an accessibility pass rather than to this row in passing.
                 */}
                <Slider
                    min={FINE_MIN}
                    max={FINE_MAX}
                    step={1}
                    value={[rotation]}
                    /*
                     * No filled track. This slider turns about a meaningful zero, so a fill from the
                     * low end would paint the left half of it solid at 0 degrees and read as a
                     * setting that had been dialled in rather than a photograph that is not turned.
                     * The design draws a bare track with a thumb, and the reference asks MUI for the
                     * same thing with `track={false}`.
                     */
                    track={false}
                    formatValue={(value) => `${value}°`}
                    onValueChange={([value]) => onRotationChange(value ?? 0)}
                />

                {/*
                 * Behind the thumb and deaf to the pointer: they are a ruler, and a mark that
                 * swallowed a click would be a mark the slider could not be dragged across.
                 */}
                <div aria-hidden className="pointer-events-none absolute inset-x-0 flex h-1.5 items-center">
                    {MARKS.map((position) => (
                        <span
                            key={position}
                            style={{ left: `${position}%` }}
                            className="absolute h-1.5 w-px -translate-x-1/2 bg-background/60"
                        />
                    ))}
                </div>

                {/*
                 * The design prints a -90/0/90 scale underneath. Only the zero is kept: the two ends
                 * restate a range the marks already show, while the zero is the one reading that means
                 * something on a control that turns about it - it is where "not turned" is. The current
                 * value rides on the thumb, so nothing else here has to be printed.
                 *
                 * **Positioned out of flow**, which is the whole reason the slider stays where it is:
                 * in flow the label would stack under the track and make *the column* the thing this
                 * row centres, leaving the track riding above the buttons. `aria-hidden` because it
                 * annotates a control that already announces its own value - a loose "0°" in the
                 * accessibility tree would read as something to operate.
                 */}
                <span
                    aria-hidden
                    className="pointer-events-none absolute top-full left-1/2 -translate-x-1/2 pt-2 pl-1 font-mono text-[11px] text-foreground-faint"
                >
                    0°
                </span>
            </div>

            <Button
                type="button"
                variant="outline"
                onClick={onReset}
                className="h-9 min-w-28 flex-none border-input bg-transparent font-normal dark:bg-transparent"
            >
                {t("common.reset")}
            </Button>
        </div>
    );
};
