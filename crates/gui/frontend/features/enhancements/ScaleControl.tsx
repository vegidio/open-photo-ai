import { type ChangeEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { clampTo, cn } from "@/lib/utils";

/** The multipliers the group offers as shortcuts, in the order the design draws them. */
const SHORTCUTS = [1, 2, 4] as const;

/** What the group is set to for a value no shortcut names. */
const CUSTOM = "custom";

// Declared once because the shortcuts and Custom differ in exactly one utility - Custom is twice as
// wide, as the design draws it - and the selected-state rules are long enough that a second copy is
// one a change can be made to only half of, invisibly until someone looks at the Custom pill.
/** How every item in the group looks, less its width. */
const SHORTCUT_ITEM =
    "h-7.5 rounded-md px-0 text-[13px] text-muted-foreground hover:bg-transparent hover:text-foreground aria-checked:bg-input aria-checked:text-foreground aria-checked:hover:bg-input";

type ScaleControlProps = {
    /** How much larger the result is, as the enhancement currently carries it. */
    value: number;
    /** The smallest and largest the selected model publishes, absent until the catalogue has arrived. */
    min?: number;
    max?: number;
    /** Called with a value already inside the published range. */
    onChange: (value: number) => void;
};

/**
 * How much larger an upscale makes the photograph: a typed multiplier, a Max shortcut, and the
 * common multipliers beside a Custom marker.
 *
 * With no bounds - the render before the catalogue arrives - nothing is clamped and Max has nothing
 * to reach, so it is unavailable.
 *
 * A typed value outside the range is **brought inside it rather than refused**. A trailing decimal
 * separator is **held rather than parsed**, and so is an empty field or a value below the minimum:
 * none of the three writes anything, and leaving the field is what settles them.
 */
export const ScaleControl = ({ value, min, max, onChange }: ScaleControlProps) => {
    // The bounds are handed in from the selected model's own published `scale` parameter rather than
    // written here, which is the property `ParameterKind` claims for itself: *"a slider built from
    // this cannot offer a value that would be refused"*. Inventing numbers where there are none would
    // be this control keeping a copy of the range after all.
    const { t } = useTranslation();

    /*
     * What the field shows, which is not always what the enhancement carries: `1.` and `` are states
     * of the input that no scale corresponds to. Re-seeded whenever the enhancement changes under
     * it - by a shortcut here, or by the stack being written from anywhere else.
     */
    const [text, setText] = useState(String(value));

    useEffect(() => setText(String(value)), [value]);

    /** Into the published range, where there is one. */
    const clamp = (scale: number) => clampTo(scale, min, max);

    const write = (scale: number) => {
        setText(String(scale));
        onChange(scale);
    };

    const onType = (event: ChangeEvent<HTMLInputElement>) => {
        const typed = event.target.value.trim();

        // Nothing to parse, and nothing to write. A trailing separator is held so `1.` survives long
        // enough for the `5` to arrive: without it the field rewrites itself to `1` between the two
        // keystrokes and 1.5 is unreachable. An empty field is left empty until a digit arrives.
        if (typed === "" || typed.endsWith(".")) {
            setText(typed);
            return;
        }

        const scale = Number.parseFloat(typed);
        if (Number.isNaN(scale)) return;

        /*
         * Below the minimum is held rather than raised, which is the same state the trailing
         * separator above is in: a prefix of the value the user is reaching for. Raising it on the
         * keystroke rewrites the field under them, so in a range starting at 2 the `1` of `12` would
         * become `2` and `12` would be unreachable - the exact failure the separator rule exists to
         * prevent, one digit earlier.
         *
         * It cannot be hit in this application today, where `Scale::MIN` is 1 and every digit is
         * already at or above it. The control does not depend on that: it reads its bounds from the
         * selected model's own published range precisely so a model that publishes a different one
         * needs no edit here, and a minimum above 1 arriving that way must not take multi-digit
         * entry with it.
         *
         * Above the maximum is *not* held, because it is not a prefix of anything: every longer
         * value is further out of range, so there is nothing to wait for and `gui-enhance`'s own
         * scenario asks for it to be brought down as it is typed.
         */
        if (min !== undefined && scale < min) {
            setText(typed);
            return;
        }

        // Brought inside the range rather than refused, which the seam does again on the far side: a
        // user who typed 12 meant "as much as you can", not "reject this".
        write(clamp(scale));
    };

    /**
     * What a held value settles at once the field is left: inside the range, as `gui-enhance` asks.
     *
     * A value below the minimum becomes the minimum - a reasonable thing to be part-way through typing
     * and not a reasonable thing to walk away from - and an empty field, or one holding a bare
     * separator, falls back to what the enhancement still carries.
     *
     * **Nothing is written where nothing moved.** The text is re-seeded instead, which is also what
     * normalizes `2.0` back to the `2` the enhancement carries.
     */
    const onLeave = () => {
        const typed = Number.parseFloat(text);
        const settled = Number.isNaN(typed) ? value : clamp(typed);

        // Writing the stack is what asks for a new run, and a field the user only tabbed through
        // should not cost one.
        if (settled === value) setText(String(value));
        else write(settled);
    };

    /* Which shortcut the current value is, or Custom for one none of them names. */
    const chosen = SHORTCUTS.find((shortcut) => shortcut === value);

    return (
        <div className="flex flex-col gap-2">
            <span className="text-[13px] text-foreground">{t("enhancements.scale.title")}</span>

            <div className="flex items-center gap-2">
                {/*
                 * `inputMode` rather than `type="number"`: the spinner a number input draws is not in
                 * the design, and its own validation would swallow the trailing separator this
                 * control exists to hold.
                 */}
                <label className="flex h-8.5 flex-1 items-center justify-between rounded-md border border-input bg-background px-2.5">
                    <input
                        inputMode="decimal"
                        value={text}
                        onChange={onType}
                        onBlur={onLeave}
                        aria-label={t("enhancements.scale.title")}
                        className="w-full min-w-0 bg-transparent font-mono text-[13px] outline-hidden"
                    />
                    {/*
                     * The same key the shortcuts below are labelled with, interpolated with nothing:
                     * the unit is what is wanted here, and a hardcoded `x` beside buttons reading
                     * `2倍` would be the field and the shortcuts spelling one unit two ways. That is
                     * the reference's own inconsistency, which hardcodes the `x` here.
                     */}
                    <span className="text-[13px] text-muted-foreground">
                        {t("enhancements.scale.shortcut", { scale: "" })}
                    </span>
                </label>

                <Button
                    variant="secondary"
                    disabled={max === undefined}
                    onClick={() => max !== undefined && write(max)}
                    className="h-8.5 flex-1 text-[13px] font-normal"
                >
                    {t("enhancements.scale.max")}
                </Button>
            </div>

            <ToggleGroup
                type="single"
                value={chosen === undefined ? CUSTOM : String(chosen)}
                onValueChange={(next: string) => {
                    // Custom names no value of its own - it reports that the typed one matches no
                    // shortcut - so pressing it changes nothing, as does pressing the shortcut that
                    // is already chosen.
                    if (next && next !== CUSTOM) write(clamp(Number(next)));
                }}
                spacing={1}
                className="flex w-full rounded-lg bg-background p-1"
            >
                {SHORTCUTS.map((shortcut) => (
                    <ToggleGroupItem key={shortcut} value={String(shortcut)} className={cn(SHORTCUT_ITEM, "flex-1")}>
                        {t("enhancements.scale.shortcut", { scale: shortcut })}
                    </ToggleGroupItem>
                ))}

                <ToggleGroupItem value={CUSTOM} className={cn(SHORTCUT_ITEM, "flex-2")}>
                    {t("enhancements.scale.custom")}
                </ToggleGroupItem>
            </ToggleGroup>
        </div>
    );
};
