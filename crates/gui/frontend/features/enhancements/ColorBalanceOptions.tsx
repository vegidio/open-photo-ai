import { useTranslation } from "react-i18next";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, withParameter } from "@/lib/enhancements";
import { IntensityOptions } from "./IntensityOptions";

type ColorBalanceOptionsProps = {
    /** The enhancement this panel is open over, which every control here reports and writes. */
    enhancement: Enhancement;
    /** The colour balance itself, as the stack carries it. */
    operation: Operation;
    /** What the catalogue publishes for colour balance: its models, their precisions and their bounds. */
    entry: FamilyEntry | undefined;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What a colour balance can be set to: which model runs it, and which way and how far it shifts the
 * photograph's colours.
 *
 * **Labelled "Bias", not "Intensity"**, although the design falls back to "Intensity" here: the parameter is the
 * same `opai::Bias` a light adjustment takes, and the label follows the type.
 *
 * The shared {@link IntensityOptions} over that `Bias`, marked at its neutral 0. See design.md D2 of
 * `add-gui-denoise`.
 */
export const ColorBalanceOptions = ({ enhancement, operation, entry, onChange }: ColorBalanceOptionsProps) => {
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
            amount={operation.parameters.bias}
            onModelChange={({ codename, precision }) => onChange({ ...operation, codename, precision })}
            onAmountChange={(bias) => onChange(withParameter(operation, "bias", bias))}
        />
    );
};
