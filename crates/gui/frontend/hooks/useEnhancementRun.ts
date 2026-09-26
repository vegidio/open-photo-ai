import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { type CropInfo, sameCrop } from "@/ipc/crop";
import { cancelEnhance, enhance, onEnhanceProgress, type RunProgress } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import type { ImageRecord } from "@/ipc/images";
import { withChoice } from "@/lib/faces";
import { report } from "@/lib/report";
import { useFileEnhancements } from "@/stores/enhancements";
import { useFaceChoice, useFacesStore } from "@/stores/faces";
import { useSettingsStore } from "@/stores/settings";

/** Where an enhanced result's pixels are, and how large they are. */
export type EnhancedImage = { identity: string; width: number; height: number };

/** What the canvas needs while and after a run: the result to draw, and what to say about the run. */
export type EnhancementRun = {
    /**
     * The result the enhanced pane draws, or `undefined` while it should draw the source.
     *
     * Absent for an image with no enhancements, for one whose first run has not landed yet, and for
     * one whose run was stopped before it produced anything.
     */
    enhanced?: EnhancedImage;
    /**
     * Whether a run is in flight, which is true from the moment one is asked for.
     *
     * What the indicator is drawn on, rather than {@link EnhancementRun.report}: a run spends its
     * first seconds loading a model and reports nothing while it does, and an indicator that waited
     * for the first report appeared long after the enhancement was added.
     */
    running: boolean;
    /** The latest report about the run in flight, or `undefined` while none has arrived yet. */
    report?: RunProgress;
    /**
     * How far along the indicator is, in `0..1`: the report's own {@link RunProgress.chainFraction}.
     *
     * One number for a face recovery too. Rust finds the faces inside the run and maps that detection
     * onto the head of the bar and the chain onto the rest (`Handover` in
     * `crates/gui/src/enhance/progress.rs`), so the bar moves forward once. Zero for a run that has
     * reported nothing yet.
     */
    fraction: number;
};

/**
 * Runs the current image's enhancements and answers what the canvas should draw and report.
 *
 * ```
 *   useEnhancementRun(file, crop)
 *       |                                        cleanup: cancelEnhance(run)
 *       enhance(identity, ops + face choice, proc, crop) ---------------+
 *       |                                                                |
 *       +-- done -> { outcome: "enhanced", identity, w, h, faces? } -----+--> enhanced pane, faces store
 *       |           { outcome: "stopped" }  -> nothing
 *       +-- rejects -> notice (errors.enhanceFailed)
 *       |
 *       +-- onEnhanceProgress -> report.run === mine ? keep : discard --> bar
 * ```
 *
 * **A face recovery finds its own faces.** The chain is asked for with the choice among faces - the
 * user's exceptions to each face's default, by key - and Rust detects inside that one run, hands the
 * recovery the faces the choice keeps, and answers every face found beside the result. So there is no
 * detect-then-enhance sequence here: one request, one progress stream, one stop. The faces answered
 * are recorded for the row to count and the picker to offer, and recording them re-runs nothing - the
 * choice is the dependency, and a detection never writes one. A detection that failed is reported
 * with its own notice; the chain still ran, over no faces. See design.md D4 and D10.
 *
 * **A photograph that has been closed is not written.** An answer settling late for a photograph the
 * window has moved off is still recorded, a warm entry for when it comes back; a photograph that was
 * *closed* has already had every owner told to forget it, and putting an entry back for it would outlive
 * the only thing that empties the store.
 *
 * **A run starts when the stack, the image, the framing, the processor or the choice of faces changes**,
 * and stops the one it replaces. A choice is one write however many boxes were clicked, so a chooser
 * opened and applied costs one run - and applying a choice that changes nothing writes nothing, so it
 * costs none. The backend enforces the displacement rather than trusting this side to ask, so the
 * explicit stop in the cleanup is for the case a new run does not cover: removing the last
 * enhancement, or moving to a photograph with none.
 *
 * **An empty stack asks for nothing.** The seam accepts an empty chain and answers with the source,
 * but that is a round trip for an answer the window already holds - so the pane is pointed at the
 * source directly, and whatever was in flight is still stopped.
 *
 * **No store.** The authoritative single-run slot is in Rust, and a store here would be a second copy
 * of it that can disagree. Nothing outside the canvas reads the run: the sidebar's rows describe what
 * *will* be run, not what is running.
 *
 * **Called once**, from the component that draws both panes and the bar.
 *
 * # What a stop leaves on screen
 *
 * A result is held until a newer one lands rather than cleared when a run starts, which is what the
 * spec asks for: a stopped run leaves the view showing what it was showing, and never a
 * partly-enhanced image. So typing in the scale field re-runs on every keystroke while the last
 * result stays on screen, instead of flashing the source between them.
 *
 * It is dropped in the three cases where holding it would be a lie: the stack emptying, the image
 * changing, and the **framing** changing. A result belongs to the photograph it was made from, which
 * is why it is held beside the identity *and the crop* it was made from rather than on its own.
 *
 * A crop takes the image-change behaviour rather than the enhancement-change behaviour, deliberately,
 * and this is where this application diverges from the reference - which keeps the previous result on
 * screen. Changing an enhancement keeps its result because the shape of what is drawn does not
 * change; changing a crop changes the pane's box in the same render, so holding the previous result
 * would draw it to the wrong proportions until the re-run landed. See design.md D10.
 *
 * **A stopped run writes nothing at all.** Slice 1's D2 made a displaced run answer `stopped`
 * whatever it returned, precisely so this can branch on the outcome and never ask whether the run it
 * is holding is still the current one. A rejection raises the notice; a stop does not, because a
 * user who changed their mind has not been told their enhancement broke.
 */
export const useEnhancementRun = (file: ImageRecord | undefined, crop?: CropInfo): EnhancementRun => {
    const { t } = useTranslation();

    const source = file?.identity;
    const operations = useFileEnhancements(file?.path);
    /*
     * The processor in force, which is all this store ever holds.
     *
     * It is a dependency of the run effect below, so a change to it cancels the run and starts a new
     * one - correct for a saved preference, and wrong for every keystroke on the way to one. While
     * the settings store held drafts this subscribed to the draft: clicking through the Performance
     * radio with the dialog open restarted the inference run on every click, and Cancel restarted it
     * once more. Nothing here had to change to fix that; the store stopped publishing drafts.
     */
    const processor = useSettingsStore((state) => state.processor);

    /*
     * The choice made among this photograph's faces, and the reason this is a dependency rather than
     * something the effect reads off the store: it is replaced whole on a write and is the same
     * reference until one happens, so the chain re-runs exactly when a choice is applied and at no other
     * time. That is the whole of "a committed change re-runs the preview, once" - it falls out of this
     * effect instead of needing a second one. See design.md D7.
     */
    const choice = useFaceChoice(source);

    /*
     * The last result, beside the photograph it was made from.
     *
     * The source travels with it rather than being cleared by an effect keyed on the identity,
     * which is what makes "a result belongs to the photograph it was made from" a fact about the
     * value rather than a race between two effects: a render that has moved to another image simply
     * does not match, so there is no frame in which one image's enhancement is drawn over another's.
     */
    const [result, setResult] = useState<{ source: string; crop?: CropInfo; image: EnhancedImage }>();
    /* What the bar is drawing: the last report this window kept. */
    const [indicator, setIndicator] = useState<RunProgress>();

    /*
     * Set in the same turn the run is asked for, so the indicator is on screen before the backend
     * has said anything at all. `mine` cannot answer this: it is a ref precisely so the progress
     * listener can read it without re-registering, and a ref does not re-render.
     */
    const [running, setRunning] = useState(false);

    // The framing is compared by value, through `sameCrop`: a result made at the framing in force is
    // this framing's result, whichever object the crop store happens to hand back for it.
    const enhanced = result && result.source === source && sameCrop(result.crop, crop) ? result.image : undefined;

    /*
     * The run this window is drawing, read by the progress listener - which is registered once and
     * must not close over a value that changes with every keystroke in the scale field.
     */
    const mine = useRef<string | undefined>(undefined);

    /*
     * The notice's own sentence, through a ref rather than as a dependency of the run effect below.
     * `t` changes identity when the language does, and a language change must not cancel an
     * inference run and start it again - which is exactly what listing it there would do. The
     * listener in `useDroppedImages` lists it because re-registering a listener costs nothing.
     */
    const notice = useRef(t);
    notice.current = t;

    /*
     * One subscription for the application, as `ipc/enhance.ts` describes: every report names its
     * own run, so a report about a run this window has abandoned is discarded rather than drawn.
     * A displaced run goes on reporting until it notices it has been stopped.
     */
    useEffect(() => {
        const listening = onEnhanceProgress((incoming) => {
            if (incoming.run !== mine.current) return;

            setIndicator(incoming);
        });

        /*
         * A subscription that could not be registered leaves the bar unreported rather than
         * unhandled. `listen` reads a global Tauri's init script installs, so it rejects wherever
         * that script has not run - a webview torn down mid-registration, and every test rendering
         * a preview without the event bridge stubbed. Caught here rather than left to the cleanup
         * below, which only runs if the component is still around to unmount: an unmount before the
         * registration settles would otherwise leave the rejection with nobody to answer it.
         *
         * The same handling the stop in the run effect below gets, and for the same reason: there
         * is nobody to propagate a dead bridge to from an effect, and the next run discovers it.
         */
        listening.catch((error: unknown) => report("subscribing to enhancement progress failed", error));

        return () => {
            // Registered asynchronously, so the unregistration has to wait for it - otherwise a
            // reloaded webview in development accumulates a listener per reload. A registration
            // that failed has nothing to unregister, and its reason is already reported above.
            listening.then((unlisten) => unlisten()).catch(() => {});
        };
    }, []);

    useEffect(() => {
        mine.current = undefined;

        if (source === undefined || operations.length === 0) {
            setRunning(false);
            setResult(undefined);
            setIndicator(undefined);
            return;
        }

        setIndicator(undefined);

        const { run, done } = enhance(source, withChoice(operations, choice), processor, crop);
        mine.current = run;
        setRunning(true);

        // Only for the run that is still current: an older run resolving after a newer one started
        // would otherwise take the bar down over a run that is still working, and wipe the newer
        // run's report out from under it.
        const ended = () => {
            if (mine.current !== run) return;

            setRunning(false);
            setIndicator(undefined);
        };

        /*
         * What a chain carrying a face recovery found, recorded for the row and the picker - only while
         * there is still a photograph to hold it for. Read off the store rather than subscribed to: what
         * this needs is the writer.
         */
        const record = (found: Face[]) => useFacesStore.getState().recordFaces(source, crop, found);

        done.then(
            (outcome) => {
                ended();

                if (outcome.outcome !== "enhanced") return;

                setResult({
                    source,
                    ...(crop && { crop }),
                    image: { identity: outcome.identity, width: outcome.width, height: outcome.height },
                });

                if (outcome.faces) record(outcome.faces);

                /*
                 * Recorded as no faces rather than left unanswered, and the chain ran anyway: refusing it
                 * would have meant a failed detection also cancelled an upscale the user asked for in the
                 * same list. The notice says what failed, as it always has. See design.md D10.
                 */
                if (outcome.facesError !== undefined) {
                    report("detecting the faces in the image failed", outcome.facesError);
                    toast.error(notice.current("errors.faceDetectFailed"));
                    record([]);
                }
            },
            (error: unknown) => {
                ended();

                // The reason in full is reported and the catalogue's own sentence goes to
                // the user, which is what every failed IPC call in this application does: a toast
                // fades before anyone copies a diagnostic out of it.
                report("enhancing the image failed", error);
                toast.error(notice.current("errors.enhanceFailed"));
            },
        );

        return () => {
            cancelEnhance(run).catch((error: unknown) =>
                // A stop that cannot be delivered means the bridge is gone, which the next run would
                // discover anyway. There is nobody to propagate it to from a cleanup.
                report("stopping the enhancement failed", error),
            );
        };
        // The crop is one of these for the same reason the image and the stack are: changing it stops
        // the run in flight and starts one over the new framing, because the run in flight is
        // producing pixels for a framing nobody is looking at any more.
        //
        // The choice among faces is one of these because it is what the run is asked to keep. It changes
        // only when a person applies one - a detection landing records faces and never writes a choice -
        // so nothing else re-runs on its account.
    }, [source, operations, processor, crop, choice]);

    return {
        running,
        // Zero for a run that has reported nothing yet, which is what the bar draws while a model
        // loads - the one honest thing to say about a run that has not spoken.
        fraction: indicator?.chainFraction ?? 0,
        ...(enhanced && { enhanced }),
        ...(indicator && { report: indicator }),
    };
};
