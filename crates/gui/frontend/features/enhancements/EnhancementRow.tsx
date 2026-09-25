import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Popover, PopoverTrigger } from "@/components/ui/popover";
import { SelectFacesDialog } from "@/features/faces/SelectFacesDialog";
import type { FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { type Enhancement, modelValue, optionFor, parameterOf, qualityLabel } from "@/lib/enhancements";
import { detectFaces } from "@/ipc/faces";
import { enabledFaces } from "@/lib/faces";
import { report } from "@/lib/report";
import { toPercent } from "@/lib/utils";
import { useImageCrop } from "@/stores/crop";
import { useFaceChoice, useFacesStore, useImageFaces } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";
import { ColorBalanceOptions } from "./ColorBalanceOptions";
import { ColorizationOptions } from "./ColorizationOptions";
import { DenoiseOptions } from "./DenoiseOptions";
import { FaceRecoveryOptions } from "./FaceRecoveryOptions";
import { LightAdjustmentOptions } from "./LightAdjustmentOptions";
import { OptionsPanel } from "./OptionsPanel";
import { SharpenOptions } from "./SharpenOptions";
import { UpscaleOptions } from "./UpscaleOptions";

type EnhancementRowProps = {
    /** The enhancement this row is of, which is what its icon and its name come from. */
    enhancement: Enhancement;
    /** What that enhancement is set to do, which is what the line beneath the name reports. */
    operation: Operation;
    /** What the catalogue publishes for this family, or `undefined` before it has arrived. */
    entry: FamilyEntry | undefined;
    /**
     * The photograph this row's enhancement is set to run over, which is what its faces are keyed by.
     *
     * Absent for a file whose bytes could not be read - which has no pixels to detect in, so its row
     * reports no count and can only ever report none.
     */
    identity?: string;
    /** Takes this enhancement off the image's stack. */
    onRemove: () => void;
    /** Puts a changed version of this enhancement back on the stack, which re-runs the preview. */
    onChange: (operation: Operation) => void;
};

// The reference's own `toFixed(3)`, and it is there to hide float noise rather than to round: a
// value typed as 1.5 and carried through a parse can arrive as 1.4999999999999998, and three places
// is past anything the control can produce. `parseFloat` then drops the zeros a whole number
// gains, so a doubling reads `2x` rather than `2.000x`.
/** How many decimal places a scale is reported to. */
const SCALE_PLACES = 3;

/**
 * One enhancement on the current image's stack: what it is, what it is set to do, and the way to
 * take it off.
 *
 * **The row itself opens the enhancement's options**: the whole row is the control, and the remove
 * button sits over its trailing edge rather than beside it.
 *
 * **The remove control is revealed on hover and on focus.**
 */
export const EnhancementRow = ({
    enhancement,
    operation,
    entry,
    identity,
    onRemove,
    onChange,
}: EnhancementRowProps) => {
    const { t } = useTranslation();
    const Icon = enhancement.icon;

    /*
     * The faces found in this photograph as it is framed, which is what a face-recovery row counts.
     * The framing is read here rather than passed in for the reason the canvas reads it: it is a
     * store keyed by the same identity, and threading it through the list would be a second copy of
     * the same lookup.
     *
     * `undefined` means *not known yet*, which is a different thing from an empty array, and the two
     * have to read differently - see `info`.
     */
    const crop = useImageCrop(identity);
    const faces = useImageFaces(identity, crop);

    /*
     * The choice made among them, which is what turns `2 Faces` into `1/2 Faces` and what the button in
     * the options panel counts down from - each face's own default, `restorable`, where the user said
     * nothing. Not compared against the framing, unlike the faces: a choice names faces by key, so one
     * recorded at another framing matches nothing.
     */
    const choice = useFaceChoice(identity);
    const chosen = enabledFaces(faces ?? [], choice).length;

    /*
     * Whether the Select faces dialog is up, held here rather than in `FaceRecoveryOptions` so the
     * dialog's lifetime is the row's rather than the panel's. Radix dismisses a popover when a modal
     * takes focus outside its content, so a dialog mounted inside the panel would unmount itself
     * mid-open along with the panel that opened it.
     */
    const [choosingFaces, setChoosingFaces] = useState(false);

    /*
     * The panel is closed deliberately as the dialog opens, rather than left to Radix: the design
     * draws one thing on screen at a time, not a panel peeking out from behind a modal.
     */
    const [panelOpen, setPanelOpen] = useState(false);

    /*
     * **The picker's own detection**: a face recovery's options opened over a photograph whose faces are
     * not known at this framing yet ask `detect_faces` for them, so Select faces has something to offer
     * without waiting for the chain. The chain finds the same faces for itself inside its own run - one
     * detection, `Detection::for_face_recovery`, served from `opai`'s run store to whichever asks second -
     * and records them when it answers; this is only for the panel a person is looking at now.
     *
     * Written only while the photograph is still open, as every late answer is: a photograph that was
     * closed has had every owner told to forget it. A failure is reported and nothing more - the chain's
     * own answer says whether the faces could be found, and it is the one that tells the user.
     */
    const lookingForFaces = panelOpen && operation.family === "face_recovery" && faces === undefined;

    useEffect(() => {
        if (!lookingForFaces || !identity) return;

        const { processor } = useSettingsStore.getState();

        detectFaces(identity, processor, crop).done.then(
            (found) => {
                if (!useFileStore.getState().files.some((open) => open.identity === identity)) return;

                useFacesStore.getState().setFaces(identity, crop, found);
            },
            (error: unknown) => report("detecting the faces for the picker failed", error),
        );
    }, [lookingForFaces, identity, crop]);

    /*
     * What the model is called and which quality it is being run at, read off the catalogue rather
     * than composed from the codename: title-casing `saopaulo` gives "Saopaulo", and the label is
     * already published beside it.
     */
    const option = optionFor(entry, modelValue(operation));

    /*
     * A model the catalogue no longer publishes - or one described before the catalogue arrived -
     * is reported by its own codename and its own precision. That is the same class of drift
     * `lib/enhancements.ts` already answers for a family the catalogue stops publishing, and the
     * same answer: say something honest rather than throw.
     */
    const name = option?.label ?? operation.codename;
    const quality = qualityLabel(t, option?.quality) ?? operation.precision;

    /*
     * Which controls the panel holds, chosen against the operation's own family for the reason the
     * info line is: every family offers something different inside one shared frame, and the reference
     * keeps the same record of family to options component.
     */
    const options = () => {
        switch (operation.family) {
            case "denoise":
                return (
                    <DenoiseOptions enhancement={enhancement} operation={operation} entry={entry} onChange={onChange} />
                );
            case "face_recovery":
                return (
                    <FaceRecoveryOptions
                        enhancement={enhancement}
                        operation={operation}
                        entry={entry}
                        found={faces?.length ?? 0}
                        chosen={chosen}
                        onSelectFaces={() => {
                            setPanelOpen(false);
                            setChoosingFaces(true);
                        }}
                        onChange={onChange}
                    />
                );
            case "light_adjustment":
                return (
                    <LightAdjustmentOptions
                        enhancement={enhancement}
                        operation={operation}
                        entry={entry}
                        onChange={onChange}
                    />
                );
            case "color_balance":
                return (
                    <ColorBalanceOptions
                        enhancement={enhancement}
                        operation={operation}
                        entry={entry}
                        onChange={onChange}
                    />
                );
            case "sharpen":
                return (
                    <SharpenOptions enhancement={enhancement} operation={operation} entry={entry} onChange={onChange} />
                );
            case "colorization":
                return (
                    <ColorizationOptions
                        enhancement={enhancement}
                        operation={operation}
                        entry={entry}
                        onChange={onChange}
                    />
                );
            case "upscale":
                return (
                    <UpscaleOptions enhancement={enhancement} operation={operation} entry={entry} onChange={onChange} />
                );
        }
    };

    // Zero where the amount is known to nobody - the operation carries none and the catalogue has not
    // arrived - which reads as the model's effect withheld rather than as a number that was never set.
    const amount = (name: string) => parameterOf(operation, entry, name) ?? 0;

    /*
     * Which sentence the line is depends on what the enhancement has to report - a scale, an
     * intensity, a face count, or nothing but the model - so it is chosen against the operation's
     * own family rather than declared beside the enhancement's name. `noImplicitReturns` is what
     * makes a new family arriving without a sentence a compile error rather than a row whose line
     * reads "NaN%".
     */
    const info = () => {
        switch (operation.family) {
            case "face_recovery":
                /*
                 * **The count is the faces actually found in the photograph as it is framed**, so
                 * framing a face out of the picture changes what this reads.
                 *
                 * Between adding the enhancement and the detection landing there is no count to
                 * report, and what this must not do is read `0 Faces` and look like an answer. So a
                 * photograph whose faces are not known yet reports the model and the quality alone,
                 * and one where none were found reports none - the two are different sentences
                 * because they are different facts. Only a known set can be counted, and only a
                 * known set can be chosen from.
                 *
                 * **Where some have been skipped it reports the ratio**, so a row cannot say `2
                 * Faces` while the run restores one.
                 */
                return faces === undefined
                    ? t("enhancements.infoModel", { name, quality })
                    : t("enhancements.infoFaces", {
                          name,
                          quality,
                          /*
                           * The reference's `facesLabel` exactly: i18next composes the two suffixes
                           * as `key_context_plural`, so `faces_partial_one` and `faces_partial_other`
                           * are selected without a second call site knowing they exist. **Pluralised
                           * on the total rather than on the chosen count**, which is what makes
                           * `0/2 Faces` read correctly - and a language with more plural forms adds
                           * keys to its own catalogue with no code change here.
                           */
                          faces: t("enhancements.faces", {
                              count: faces.length,
                              enabled: chosen,
                              ...(chosen !== faces.length && { context: "partial" }),
                          }),
                      });
            case "denoise":
            case "sharpen":
                // Shared by the two families that carry a strength, and kept apart from the bias cases below:
                // the field they read differs.
                return t("enhancements.info", { name, quality, intensity: toPercent(amount("strength")) });
            case "light_adjustment":
            case "color_balance":
                // A negative bias keeps its sign: the direction is what it changes.
                return t("enhancements.info", { name, quality, intensity: toPercent(amount("bias")) });
            case "colorization":
                // The model and the quality alone: there is no amount to report. Its own case rather than a
                // fall-through onto face recovery, whose case counts faces.
                return t("enhancements.infoModel", { name, quality });
            case "upscale":
                return t("enhancements.infoScale", {
                    name,
                    quality,
                    // 1x where the scale is known to nobody: a factor that changes nothing.
                    scale: Number.parseFloat((parameterOf(operation, entry, "scale") ?? 1).toFixed(SCALE_PLACES)),
                });
        }
    };

    return (
        <div className="group relative border-b border-border" data-slot="enhancement-row">
            <Popover open={panelOpen} onOpenChange={setPanelOpen}>
                <PopoverTrigger asChild>
                    {/*
                     * The whole row is the trigger, which is the reference's arrangement and the
                     * design's (screen 14).
                     *
                     * `min-h` rather than a fixed height: the two lines are 13px and 12px in every
                     * language, but a model name long enough to wrap has to be able to. The panel is
                     * anchored against `ENHANCEMENT_ROW_HEIGHT`, which is this `min-h-13`.
                     *
                     * `data-[state=open]:bg-secondary` beside the hover, because the design paints an
                     * open row exactly as it paints a hovered one (screen 11) - a row whose panel is
                     * up with nothing marking it is a panel with no visible source.
                     */}
                    <button
                        type="button"
                        className="flex min-h-13 w-full items-center gap-3 py-2 pr-10 pl-4 text-left hover:bg-secondary data-[state=open]:bg-secondary"
                    >
                        <Icon className="size-5 flex-none" />

                        <span className="flex min-w-0 flex-col gap-px">
                            <span className="text-[13px]">{t(enhancement.nameKey)}</span>
                            <span className="text-xs text-foreground-dim italic" data-slot="enhancement-info">
                                {info()}
                            </span>
                        </span>
                    </button>
                </PopoverTrigger>

                <OptionsPanel title={t(enhancement.nameKey)}>{options()}</OptionsPanel>
            </Popover>

            {/*
             * A sibling of the popover rather than a child of it, so closing the panel above does not
             * take the dialog down with it. Mounted only for the family that offers it, which is the
             * same carve-out the info line and the options panel already make.
             */}
            {operation.family === "face_recovery" && (
                <SelectFacesDialog open={choosingFaces} onClose={() => setChoosingFaces(false)} />
            )}

            <Button
                variant="secondary"
                size="icon-sm"
                aria-label={t("enhancements.remove")}
                onClick={onRemove}
                // Hover alone is the design's rule and the reference's, and it is what keeps a list of
                // several reading as a list rather than as a column of buttons - but hover is not
                // reachable from a keyboard, so the control is in the tab order and shows itself when
                // it is focused. Drawn at zero opacity rather than not drawn at all, which is what
                // keeps it focusable in the first place.
                className="absolute top-1/2 right-2 size-6.5 -translate-y-1/2 bg-input opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
            >
                <X className="size-4" />
            </Button>
        </div>
    );
};
