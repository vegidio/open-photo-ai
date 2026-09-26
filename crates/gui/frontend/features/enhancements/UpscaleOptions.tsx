import { Separator } from "@/components/ui/separator";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, parameterOf, publishedRange, withParameter } from "@/lib/enhancements";
import { OperationModelTray } from "./ModelTray";
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
    operation: Operation;
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
    // The selected model's rather than the family's - see `publishedRange`.
    const range = publishedRange(entry, operation.codename, SCALE_PARAMETER);

    return (
        <>
            <OperationModelTray enhancement={enhancement} operation={operation} entry={entry} onChange={onChange} />

            <Separator />

            <ScaleControl
                // 1x where neither the operation nor the catalogue knows a scale: a factor that changes nothing.
                value={parameterOf(operation, entry, SCALE_PARAMETER) ?? 1}
                {...(range && { min: range.min, max: range.max })}
                onChange={(scale) => onChange(withParameter(operation, SCALE_PARAMETER, scale))}
            />
        </>
    );
};
