import { useTranslation } from "react-i18next";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, withParameter } from "@/lib/enhancements";
import { IntensityOptions } from "./IntensityOptions";

type DenoiseOptionsProps = {
    /** The enhancement this panel is open over, which every control here reports and writes. */
    enhancement: Enhancement;
    /** The denoise itself, as the stack carries it. */
    operation: Operation;
    /** What the catalogue publishes for denoise: its models, their precisions and their bounds. */
    entry: FamilyEntry | undefined;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What a denoise can be set to: which model runs it, and how strongly its output is applied.
 *
 * **Labelled "Strength", not "Intensity"**, although the reference passes no label and so shows "Intensity":
 * screen 12 names it Strength, and the parameter is `opai::Strength` - the label follows the type.
 *
 * The shared {@link IntensityOptions} over that `Strength`, marked at 100, where the model's own output is applied
 * unchanged. See design.md D2 and D3 of `add-gui-denoise`.
 */
export const DenoiseOptions = ({ enhancement, operation, entry, onChange }: DenoiseOptionsProps) => {
    const { t } = useTranslation();

    return (
        <IntensityOptions
            enhancement={enhancement}
            entry={entry}
            codename={operation.codename}
            precision={operation.precision}
            parameter="strength"
            label={t("enhancements.strength")}
            mark={100}
            amount={operation.parameters.strength}
            onModelChange={({ codename, precision }) => onChange({ ...operation, codename, precision })}
            onAmountChange={(strength) => onChange(withParameter(operation, "strength", strength))}
        />
    );
};
