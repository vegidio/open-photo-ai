import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { familyEntry } from "@/hooks/useCatalogue";
import i18n from "@/i18n";
import { type Suggestion, suggest } from "@/ipc/autopilot";
import { catalogue } from "@/ipc/catalogue";
import type { CropInfo } from "@/ipc/crop";
import type { Operation } from "@/ipc/enhance";
import { detectFaces } from "@/ipc/faces";
import type { ImageRecord } from "@/ipc/images";
import { familiesWhere, suggestedOperation } from "@/lib/enhancements";
import { noFaceToRestore } from "@/lib/faces";
import { track } from "@/lib/faro";
import { report } from "@/lib/report";
import { useAutopilotStore } from "@/stores/autopilot";
import { useImageCrop } from "@/stores/crop";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFacesStore } from "@/stores/faces";
import { useCurrentFile, useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";

/** Whether a rejection is an analysis this side withdrew, which is never reported. */
const isStopped = (error: unknown) =>
    typeof error === "object" && error !== null && "kind" in error && error.kind === "stopped";

/** Whether `path` is the photograph the window is showing now. */
const isCurrent = (path: string) => {
    const { files, currentIndex } = useFileStore.getState();

    return files.at(currentIndex)?.path === path;
};

/** What came of one analysis. See {@link analyse}. */
export type AnalysisOutcome = "added" | "stopped" | { failed: unknown };

/**
 * Analyses one photograph at one framing and adds what it calls for to its stack.
 *
 * ```
 *   suggest(identity, processor, allowed families, crop) -> begin(path, run, crop)
 *       |
 *       +-- stopped   -> nothing
 *       +-- rejects   -> notice (errors.autopilotFailed), decline if current
 *       +-- answers   -> ours()?
 *              |
 *              +-- face recovery among them -> detectFaces(identity, processor, crop) -> ours()?
 *              |        +-- answers -> setFaces(identity, crop, faces); drop it if noFaceToRestore
 *              |        +-- rejects -> keep it, record nothing
 *              |
 *              +-- catalogue() -> ours()? -> addEnhancements(path, operations)
 *   finally: end(path, run)
 * ```
 *
 * **Every store is read through `getState()` at the step that needs it**, not captured at render: the
 * exclusions and the processor at the moment of asking, the models at the moment of building.
 *
 * **`ours()` is checked after every await.** It holds while the photograph's entry still names this run, and a
 * close, a switch-off or a reframe removes it - even where the backend's answer beat the stop across the
 * boundary. A discarded answer writes nothing at all.
 *
 * **The stack is written before the entry is ended, in the same tick.** Ending first would leave one render in
 * which the photograph has neither a stack nor an analysis, and the trigger would start a second one. `end` runs
 * on every exit through the `finally`, and is a no-op where a stop already removed the entry.
 *
 * **The follow-up detection is Autopilot's, not the canvas's.** The analysis just ran the same detection at the
 * same precision on the same framed identity, so it is a run-store hit. Its progress reports carry a run name
 * `useEnhancementRun` never minted, so the canvas indicator ignores them; the analysing row stays up through it,
 * because the entry does. The faces are recorded whether or not the suggestion survives, so the run that follows
 * - or a face recovery the user adds later at this framing - does not detect again.
 *
 * A photograph with no identity cannot be analysed and is never asked about; it answers a failure with no cause.
 *
 * **Who asked decides who is told.** The canvas trigger asks with `notify` on, and ignores the answer: a failure is a
 * notice, and the current photograph is declined. An export asks with it off, because the failure is reported on its
 * row and the photograph need not be current: a failure is only the answer. Only an analysis with `notify` on sends
 * the `autopilot_run` and `enhancement_added` events, because only it is the user's.
 *
 * Answers what came of it: `added` once the stack is written, `stopped` where the analysis was stopped or its answer
 * discarded, and `{ failed }` with what it failed with.
 */
export const analyse = async (
    file: ImageRecord,
    crop: CropInfo | undefined,
    { notify }: { notify: boolean } = { notify: true },
): Promise<AnalysisOutcome> => {
    const { path, identity } = file;
    if (identity === undefined) return { failed: undefined };

    const { autopilotExcluded, processor } = useSettingsStore.getState();
    // Every enhancement offered, less the ones switched off. An empty set is still sent: it answers nothing,
    // which is what the user asked for, and it is not a failure.
    const families = familiesWhere((family) => !autopilotExcluded.includes(family));

    const { run, done } = suggest(identity, processor, families, crop);
    useAutopilotStore.getState().begin(path, run, crop);

    const ours = () => useAutopilotStore.getState().analysing.get(path)?.run === run;

    try {
        let suggestions: Suggestion[] = await done;
        if (!ours()) return "stopped";

        if (suggestions.some((suggestion) => suggestion.family === "face_recovery")) {
            try {
                const faces = await detectFaces(identity, processor, crop).done;
                if (!ours()) return "stopped";

                // The crop reference `begin` captured, which `ours()` guarantees is still the crop store's
                // current one - so `useImageFaces` matches it.
                useFacesStore.getState().setFaces(identity, crop, faces);

                if (noFaceToRestore(faces)) {
                    suggestions = suggestions.filter((suggestion) => suggestion.family !== "face_recovery");
                }
            } catch (error) {
                // The suggestion is kept and nothing is recorded: the run path then detects when it runs, and
                // reports its own failure there, as it does for a face recovery added by hand.
                report("detecting the faces for an Autopilot suggestion failed", error);
                if (!ours()) return "stopped";
            }
        }

        const entries = await catalogue();
        if (!ours()) return "stopped";

        const { models } = useSettingsStore.getState();
        const operations = suggestions
            .map((suggestion) =>
                suggestedOperation(
                    suggestion,
                    familyEntry(entries, suggestion.family),
                    models[suggestion.family],
                    file,
                ),
            )
            .filter((operation): operation is Operation => operation !== undefined);

        useEnhancementStore.getState().addEnhancements(path, operations);

        // The user's analysis only: an export's own is not a decision anyone made about this photograph.
        if (notify) {
            for (const { family } of operations) track("enhancement_added", { family, source: "autopilot" });
            track("autopilot_run", { count: operations.length });
        }

        return "added";
    } catch (error) {
        // A stop is the user's own word, and an answer nobody is waiting for any more is not theirs to hear.
        if (isStopped(error) || !ours()) return "stopped";

        report("the Autopilot analysis failed", error);
        if (!notify) return { failed: error };

        toast.error(i18n.t("errors.autopilotFailed"));

        // Only the current photograph: one the user moved off is asked about again when they come back to it,
        // which is already "becoming current". See design.md D4.
        if (isCurrent(path)) useAutopilotStore.getState().decline(path);

        return { failed: error };
    } finally {
        useAutopilotStore.getState().end(path, run);
    }
};

/**
 * An analysis of one photograph for an export: the one already in flight, where there is one, or a new one.
 *
 * **One in flight is waited for, not asked again**, whoever asked it - the canvas trigger, usually. Its entry is
 * removed once its stack is written, in the same tick, so the answer is `added` where the photograph then has a
 * stack and a failure otherwise. That failure carries no cause: the trigger that asked reported its own already.
 *
 * Otherwise it is {@link analyse} with `notify` off. That registers in the same store, so the canvas trigger, seeing
 * the entry, does not ask a second time for the current photograph.
 */
export const awaitAnalysis = (file: ImageRecord, crop: CropInfo | undefined): Promise<AnalysisOutcome> => {
    const { path } = file;

    if (!useAutopilotStore.getState().analysing.has(path)) return analyse(file, crop, { notify: false });

    return new Promise((resolve) => {
        const unsubscribe = useAutopilotStore.subscribe((state) => {
            if (state.analysing.has(path)) return;

            unsubscribe();
            resolve(useEnhancementStore.getState().enhancements.has(path) ? "added" : { failed: undefined });
        });
    });
};

/**
 * Analyses the photograph the user is looking at, when Autopilot is on and it has never had a list.
 *
 * In order, each time any of its inputs changes:
 *
 * 1. **Autopilot off**: stop every analysis in flight. That is the switch-off stop.
 * 2. **Nothing to analyse**: no file, or no identity.
 * 3. **In flight**: at the same framing, nothing to do - moving off and back asks nothing new. At another
 *    framing, stop it (the reframe stop), whether or not the photograph has a stack by now.
 * 4. **Has a stack** (emptied ones included): nothing more. After a reframe stop this is a photograph the user
 *    added to by hand during the analysis, which has had a list and is not asked about again.
 * 5. **Declined**: the current photograph's analysis just failed; wait until it stops being current or
 *    Autopilot is switched on again.
 * 6. **Analyse.**
 *
 * **Nothing is stopped from a cleanup.** Every re-run of the effect is a dependency changing, and most of those -
 * the stack being written, moving off, the entry itself appearing - must not stop anything. The reference's
 * comment records what the other way cost it. The three real stops are closing the photograph (the store's file
 * owner), step 1 and step 3, each an explicit call.
 *
 * **Called once**, from the sidebar.
 */
export const useAutopilot = () => {
    const file = useCurrentFile();
    const path = file?.path;

    const autopilot = useEnhancementStore((state) => state.autopilot);
    const hasStack = useEnhancementStore((state) => path !== undefined && state.enhancements.has(path));
    const crop = useImageCrop(file?.identity);
    const inFlight = useAutopilotStore((state) => (path === undefined ? undefined : state.analysing.get(path)));

    /*
     * The current photograph and the switch as the last pass saw them, which is what tells this pass that the
     * photograph has just become current or Autopilot has just been switched on - the two events that lift a
     * decline. A ref rather than state, because it must not be the thing that re-renders.
     */
    const seen = useRef<{ path?: string; autopilot: boolean }>(undefined);

    useEffect(() => {
        const store = useAutopilotStore.getState();

        const previous = seen.current;
        seen.current = { ...(file && { path: file.path }), autopilot };

        if (previous?.path !== file?.path || (autopilot && !previous?.autopilot)) store.clearDeclined();

        if (!autopilot) {
            store.stopAll();
            return;
        }

        if (!file || file.identity === undefined) return;

        // Ahead of the stack check: a stack added to by hand during the analysis does not exempt it from the
        // reframe stop, or its answer would land measured on a framing that is gone.
        if (inFlight) {
            // By reference, as every other reader of the crop store compares it.
            if (inFlight.crop === crop) return;

            store.stop(file.path);
        }

        if (hasStack) return;

        // Read again rather than off `store`, which is the state before the decline above was lifted.
        if (useAutopilotStore.getState().declined === file.path) return;

        void analyse(file, crop, { notify: true });
    }, [file, autopilot, hasStack, crop, inFlight]);
};
