import { type ChangeEvent, type KeyboardEvent, useEffect, useState } from "react";
import { ArrowLeftRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { MIN_CROP_SIZE } from "@/lib/constants";

/** A field's text as a number, or `NaN` for anything that is not one - including an emptied field. */
const toInt = (value: string) => Number.parseInt(value.trim(), 10);

/**
 * One dimension of the rectangle, in the photograph's own pixels.
 *
 * **The field is authoritative while it is focused.** It holds its own text and mirrors the framing
 * only when the user is not in it; on blur the display goes back to what the framing actually is.
 *
 * Committed on every keystroke rather than on blur, so the rectangle tracks what is being typed;
 * anything that is not a number is skipped rather than committed. An emptied field that is then left
 * falls back to the minimum.
 */
const DimensionField = ({
    label,
    name,
    value,
    onCommit,
}: {
    label: string;
    name: string;
    value: number;
    onCommit: (value: number) => void;
}) => {
    const [input, setInput] = useState(String(value));
    const [focused, setFocused] = useState(false);

    // Mirroring only while unfocused is what stops the live stencil from clobbering a half-typed
    // number - `25` on the way to `2560` is a legal width, so the box resizes, so the framing comes
    // back, so the field would be rewritten to `25` under the caret. It is also what contains the
    // one-pixel snap a reduced rendition costs: the display goes back to what the framing actually is
    // once, on blur, rather than fighting each keystroke.
    useEffect(() => {
        if (!focused) setInput(String(value));
    }, [value, focused]);

    const onChange = (event: ChangeEvent<HTMLInputElement>) => {
        setInput(event.target.value);

        // Skipping a non-number is what lets a field be emptied on the way to another value.
        const parsed = toInt(event.target.value);
        if (!Number.isNaN(parsed)) onCommit(parsed);
    };

    return (
        <label className="flex h-8.5 min-w-0 flex-1 items-center gap-1.5 rounded-md border border-input bg-background px-2">
            <span className="flex-none text-xs text-foreground-dim">{label}</span>
            <input
                name={name}
                aria-label={label}
                inputMode="numeric"
                value={input}
                onFocus={() => setFocused(true)}
                onChange={onChange}
                // A rectangle with no width is not a framing - Rust refuses one outright - so an emptied
                // field resolves to the minimum rather than being left to mean nothing.
                onBlur={() => {
                    if (Number.isNaN(toInt(input))) onCommit(MIN_CROP_SIZE);
                    setFocused(false);
                }}
                // Enter blurs rather than submitting: there is no form here, and blurring is what
                // runs the snap-back the field is holding off while it has focus.
                onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
                    if (event.key === "Enter") event.currentTarget.blur();
                }}
                className="w-full min-w-0 bg-transparent font-mono text-[13px] outline-hidden"
                data-slot={`crop-${name}`}
            />
        </label>
    );
};

/**
 * The `w` and `h` fields with the swap between them, as screen 18 draws them.
 *
 * The labels are the catalogue's `crop.width` and `crop.height`, which are the single characters the
 * design draws rather than words, and they are the field's accessible name as well as its visible
 * prefix.
 */
export const CropDimensions = ({
    width,
    height,
    onWidthCommit,
    onHeightCommit,
    onSwap,
}: {
    width: number;
    height: number;
    onWidthCommit: (value: number) => void;
    onHeightCommit: (value: number) => void;
    onSwap: () => void;
}) => {
    const { t } = useTranslation();

    return (
        <div className="flex items-center gap-2" data-slot="crop-dimensions">
            <DimensionField label={t("crop.width")} name="width" value={width} onCommit={onWidthCommit} />

            <Button
                type="button"
                variant="secondary"
                size="icon-sm"
                aria-label={t("crop.swap")}
                onClick={onSwap}
                className="flex-none [&_svg:not([class*='size-'])]:size-4.5"
            >
                <ArrowLeftRight />
            </Button>

            <DimensionField label={t("crop.height")} name="height" value={height} onCommit={onHeightCommit} />
        </div>
    );
};
