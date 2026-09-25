import { Separator } from "@/components/ui/separator";
import type { FamilyEntry, Precision } from "@/ipc/catalogue";
import { type Enhancement, type ModelOption, modelValue, optionFor, publishedRange } from "@/lib/enhancements";
import { toPercent } from "@/lib/utils";
import { IntensityControl } from "./IntensityControl";
import { ModelTray } from "./ModelTray";

/** How many percent one unit is: the wire speaks units, this panel speaks percent. */
const PERCENT = 100;

type IntensityOptionsProps = {
    /** The enhancement this panel is open over, which every control here reports and writes. */
    enhancement: Enhancement;
    /** What the catalogue publishes for the family: its models, their precisions and their bounds. */
    entry: FamilyEntry | undefined;
    /** The selected model, as the operation carries it. */
    codename: string;
    /** The selected model's build, as the operation carries it. */
    precision: Precision;
    /**
     * What the library calls the family's amount - its own `Bias::NAME` or `Strength::NAME`, on the wire - which
     * is the parameter the bounds are read from. Named rather than found by kind, for `UpscaleOptions`' reason.
     */
    parameter: "bias" | "strength";
    /** The control's label, already translated. */
    label: string;
    /** The percentage marked beneath the track: the amount at which the model's effect is its neutral one. */
    mark: number;
    /**
     * The amount the operation carries, in the wire's unit value - or `undefined` where it carries none,
     * which draws the selected model's published default.
     */
    amount: number | undefined;
    /** A model was chosen; the caller writes its codename and precision into its own operation. */
    onModelChange: (option: ModelOption) => void;
    /** An amount was committed, in the wire's unit value; the caller writes it into its own field. */
    onAmountChange: (amount: number) => void;
};

/**
 * A model and one amount: the options every family whose parameter is a single unit value draws - a model tray, a
 * separator, and a typed percentage above a slider.
 *
 * **Nothing is written on open**, for `UpscaleOptions`' reason: writing the stack asks for a new run.
 *
 * **The conversion between the wire's unit value and the control's percentage happens here**, both ways - the
 * control knows nothing about any parameter. The bounds are the **selected model's** own published range for
 * `parameter`, times a hundred, because `VariantEntry.parameters` is per model.
 *
 * **It writes back through callbacks rather than building the operation**, so each wrapper names the parameter it
 * writes in one line of its own. A chosen model keeps the amount, since only the codename and precision are handed
 * back. See design.md D2.
 */
export const IntensityOptions = ({
    enhancement,
    entry,
    codename,
    precision,
    parameter,
    label,
    mark,
    amount,
    onModelChange,
    onAmountChange,
}: IntensityOptionsProps) => {
    const range = publishedRange(entry, codename, parameter);

    const chooseModel = (chosen: string) => {
        const option = optionFor(entry, chosen);
        if (option) onModelChange(option);
    };

    return (
        <>
            <ModelTray
                enhancement={enhancement}
                entry={entry}
                value={modelValue({ codename, precision })}
                onChange={chooseModel}
            />

            <Separator />

            <IntensityControl
                label={label}
                // Zero where neither the operation nor the catalogue knows an amount: the model's effect withheld.
                value={toPercent(amount ?? range?.default ?? 0)}
                {...(range && { min: range.min * PERCENT, max: range.max * PERCENT })}
                mark={mark}
                onChange={(percent) => onAmountChange(percent / PERCENT)}
            />
        </>
    );
};
