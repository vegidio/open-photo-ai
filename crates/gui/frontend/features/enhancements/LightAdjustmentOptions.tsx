import { useTranslation } from "react-i18next";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import type { Enhancement } from "@/lib/enhancements";
import { IntensityOptions } from "./IntensityOptions";

type LightAdjustmentOptionsProps = {
    /** The enhancement this panel is open over, which every control here reports and writes. */
    enhancement: Enhancement;
    /** The light adjustment itself, as the stack carries it. */
    operation: Extract<Operation, { family: "light_adjustment" }>;
    /** What the catalogue publishes for light adjustment: its models, their precisions and their bounds. */
    entry: FamilyEntry | undefined;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What a light adjustment can be set to: which model runs it, and which way and how far it shifts the
 * photograph.
 *
 * The shared {@link IntensityOptions} over the library's `Bias`, marked at its neutral 0. What is this family's
 * alone is the parameter, the label and the field the amount is written to. See design.md D2 of `add-gui-denoise`.
 */
export const LightAdjustmentOptions = ({ enhancement, operation, entry, onChange }: LightAdjustmentOptionsProps) => {
    const { t } = useTranslation();

    return (
        <IntensityOptions
            enhancement={enhancement}
            entry={entry}
            codename={operation.codename}
            precision={operation.precision}
            parameter="bias"
            label={t("enhancements.bias")}
            mark={0}
            amount={operation.bias}
            onModelChange={({ codename, precision }) => onChange({ ...operation, codename, precision })}
            onAmountChange={(bias) => onChange({ ...operation, bias })}
        />
    );
};
