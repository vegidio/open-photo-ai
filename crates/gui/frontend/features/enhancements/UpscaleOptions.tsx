import { Separator } from "@/components/ui/separator";
import type { FamilyEntry, ParameterEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, modelValue, optionFor } from "@/lib/enhancements";
import { ModelTray } from "./ModelTray";
import { ScaleControl } from "./ScaleControl";

// Named rather than found by kind, because a variant may publish more than one range - face
// recovery already publishes a fidelity beside its faces - and "the only range" would then be the
// wrong one. The spelling is the library's own `Scale::NAME`, on the wire.
/**
 * What the library calls an upscale's multiplier, which is the parameter this panel's bounds come
 * from.
 */
const SCALE_PARAMETER = "scale";

type UpscaleOptionsProps = {
    /** The enhancement this panel is open over, which every control here reports and writes. */
    enhancement: Enhancement;
    /** The upscale itself, as the stack carries it. */
    operation: Extract<Operation, { family: "upscale" }>;
    /** What the catalogue publishes for upscale: its models, their precisions and their bounds. */
    entry: FamilyEntry | undefined;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What an upscale can be set to: which model runs it, and how much larger the result is.
 *
 * **Nothing is written on open.** Both controls write only when they are operated, so opening a
 * panel to see what an enhancement is set to never cancels the run in flight - which it would,
 * since writing the stack is what asks for a new one.
 *
 * The scale's bounds are the **selected model's** own published range.
 */
export const UpscaleOptions = ({ enhancement, operation, entry, onChange }: UpscaleOptionsProps) => {
    const value = modelValue(operation);

    // The selected model's rather than the family's, because `VariantEntry.parameters` is per model:
    // two models of one family may disagree, and a control built from a family-wide list would offer
    // one of them a value its sibling refuses.
    const variant = entry?.variants.find((published) => published.codename === operation.codename);
    // The predicate narrows as well as finds: `ParameterEntry` is a union, and the bounds only
    // exist on the `range` arm.
    const range = variant?.parameters.find(
        (parameter): parameter is ParameterEntry & { kind: "range" } =>
            parameter.name === SCALE_PARAMETER && parameter.kind === "range",
    );

    /*
     * A chosen model is resolved to the option it names rather than parsed: the option carries both
     * halves as fields, so nothing here knows how the value is composed. One the catalogue no longer
     * publishes resolves to nothing and is ignored, which leaves the enhancement on the model it is
     * already running.
     */
    const chooseModel = (chosen: string) => {
        const option = optionFor(entry, chosen);
        if (!option) return;

        onChange({ ...operation, codename: option.codename, precision: option.precision });
    };

    return (
        <>
            <ModelTray enhancement={enhancement} entry={entry} value={value} onChange={chooseModel} />

            <Separator />

            <ScaleControl
                value={operation.scale}
                {...(range && { min: range.min, max: range.max })}
                onChange={(scale) => onChange({ ...operation, scale })}
            />
        </>
    );
};
