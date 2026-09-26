import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import type { Enhancement } from "@/lib/enhancements";
import { OperationModelTray } from "./ModelTray";

type ColorizationOptionsProps = {
    /** The enhancement this panel is open over, which the tray reports and writes. */
    enhancement: Enhancement;
    /** The colorization itself, as the stack carries it. */
    operation: Operation;
    /** What the catalogue publishes for colorization: its models and their precisions. */
    entry: FamilyEntry | undefined;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What a colorization can be set to: which model runs it, and nothing else.
 *
 * The reference's `OptionsColorization`, whose own comment is the reason: *"Colorization has no per-run
 * amount, so only the model half of the hook is used."* No separator either - the reference's divider
 * sits between a model selector and a second control, and there is no second control here. Not
 * `IntensityOptions` with the amount made optional: that component is a model and one amount,
 * and the tray is already its own component. See design.md D3 of `add-gui-colorization`.
 *
 * **Nothing is written on open.** The tray writes only when it is operated, so opening the panel to see
 * which model is in use never cancels the run in flight.
 */
export const ColorizationOptions = ({ enhancement, operation, entry, onChange }: ColorizationOptionsProps) => (
    <OperationModelTray enhancement={enhancement} operation={operation} entry={entry} onChange={onChange} />
);
