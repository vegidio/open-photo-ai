import { Trans, useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, modelValue, optionFor } from "@/lib/enhancements";
import { ModelTray } from "./ModelTray";

type FaceRecoveryOptionsProps = {
    /** The enhancement this panel is open over, which the tray reports and writes. */
    enhancement: Enhancement;
    /** The face recovery itself, as the stack carries it. */
    operation: Extract<Operation, { family: "face_recovery" }>;
    /** What the catalogue publishes for face recovery: its models and their precisions. */
    entry: FamilyEntry | undefined;
    /**
     * How many faces were found in this photograph as it is framed.
     *
     * Zero both for one with nobody in it and for one whose faces are not known yet, because the
     * control below is unavailable for either: there is nothing to choose among in both cases.
     */
    found: number;
    /** How many of those the run will restore, which is what the control reports. */
    chosen: number;
    /** Opens the Select faces dialog, which the row owns - see `EnhancementRow`. */
    onSelectFaces: () => void;
    /** Writes the enhancement back to the stack, which is what re-runs the preview. */
    onChange: (operation: Operation) => void;
};

/**
 * What a face recovery can be set to: which model runs it, and which of the faces it restores.
 *
 * **The Faces block is a way in rather than the chooser itself** (screen 13): a rule, the heading, the
 * centred two-line help and a button carrying the chosen count. The button is unavailable for a
 * photograph with no faces found in it, rather than opening on an empty picture.
 *
 * **Nothing is written on open.** The tray writes only when it is operated and the button writes
 * nothing at all, so opening a panel to see which model an enhancement is set to never cancels the
 * run in flight - which it would, since writing the stack is what asks for a new one.
 */
export const FaceRecoveryOptions = ({
    enhancement,
    operation,
    entry,
    found,
    chosen,
    onSelectFaces,
    onChange,
}: FaceRecoveryOptionsProps) => {
    /*
     * **The fidelity is not among the options, deliberately.** It is fixed at maximum and offered
     * nowhere: that is what the reference hard-codes for every run and what an untouched control would
     * carry, so drawing one would be a slider with a single legal position. It does not even cross the
     * wire - see `crates/gui/src/enhance/operation.rs`.
     */
    const { t } = useTranslation();

    /*
     * Resolved rather than parsed, as `UpscaleOptions`' `chooseModel` explains.
     *
     * The faces are left exactly as they are - they belong to the pixels, not to the model, and the
     * stack's copy carries none in any case. `useEnhancementRun` puts them in on the way to the run.
     */
    const chooseModel = (value: string) => {
        const option = optionFor(entry, value);
        if (!option) return;

        onChange({ ...operation, codename: option.codename, precision: option.precision });
    };

    return (
        <>
            <ModelTray enhancement={enhancement} entry={entry} value={modelValue(operation)} onChange={chooseModel} />

            <Separator />

            <div className="flex flex-col gap-2.5">
                <span className="text-[13px]">{t("enhancements.faceSelector.title")}</span>

                {/*
                 * Through `<Trans>` rather than `t`, because the sentence carries a `<br/>`: the break
                 * is part of the centred two-line layout, so it sits in the catalogue where a
                 * translator can move it or drop it. `<br/>` is one of i18next's default kept nodes,
                 * so no `components` prop is needed - the reference's own arrangement.
                 */}
                <p className="text-center text-[13px] text-muted-foreground leading-normal">
                    <Trans i18nKey="enhancements.faceSelector.help" />
                </p>

                {/*
                 * The face boxes are drawn in the dialog this opens and nowhere else - not over the
                 * canvas, not over the enhanced pane and not over the sidebar's miniature - because
                 * the canvas is where a user judges the photograph, and boxes standing over the faces
                 * are exactly what stops them seeing the restoration they asked for.
                 */}
                <Button
                    variant="secondary"
                    className="h-9 border border-input font-normal text-[13px]"
                    disabled={found === 0}
                    onClick={onSelectFaces}
                >
                    {t("enhancements.faceSelector.button", { count: chosen })}
                </Button>
            </div>
        </>
    );
};
