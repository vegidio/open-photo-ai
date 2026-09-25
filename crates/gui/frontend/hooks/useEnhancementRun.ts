import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import type { CropInfo } from "@/ipc/crop";
import { cancelEnhance, enhance, onEnhanceProgress, type RunProgress } from "@/ipc/enhance";
import { detectFaces, type Face } from "@/ipc/faces";
import type { ImageRecord } from "@/ipc/images";
import { enabledFaces, withFaces } from "@/lib/faces";
import { report } from "@/lib/report";
import { useFileEnhancements } from "@/stores/enhancements";
import { useFacesStore, useImageFaces, useSkippedFaces } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";

/**
 * How much of the indicator the detection owns, before the chain that asked for it starts.
 *
 * The window draws **one** bar over what is underneath it two runs - the detection `faces.rs` runs
 * and the chain `enhance` runs - and each of those reports its own `0..1` and lands on exactly 1.
 * Drawn as they arrive that is two sweeps: the bar fills, empties and fills again, which reads as
 * the enhancement having been applied twice.
 *
 * So the detection is mapped onto the head of the range and the chain onto what it leaves. The
 * figure is the reference's own `progressAfterDetect`, which reserved exactly this fifth inside its
 * face recovery because its package acquired the detector itself; here detection is an operation of
 * its own (see `crates/opai/src/models/face_recovery/restore.rs`), so the split belongs in the one
 * place that knows the two runs are a single thing to the person watching.
 *
 * **A chain that needed no detection owns the whole range**, so an ordinary enhancement is
 * unaffected and a re-run over faces already known takes the bar back in full.
 */
const DETECTION_SHARE = 0.2;

/** The part of the indicator a run owns: where it starts, and how much of the bar it covers. */
type Share = { base: number; span: number };

/** A run nothing preceded, which is every chain but the one a detection was just run for. */
const WHOLE_BAR: Share = { base: 0, span: 1 };

/** A detection, which owns the head of the range. */
const DETECTING: Share = { base: 0, span: DETECTION_SHARE };

/** The chain a detection was just run for, which owns what that detection left. */
const AFTER_DETECTION: Share = { base: DETECTION_SHARE, span: 1 - DETECTION_SHARE };

/** Where an enhanced result's pixels are, and how large they are. */
export type EnhancedImage = { identity: string; width: number; height: number };

/** What the indicator draws: the report it is naming, and where on the bar that report puts it. */
type Indicator = { report: RunProgress; fraction: number };

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
    /**
     * The latest report about the run in flight, or `undefined` while none has arrived yet.
     *
     * Held across the handover from a detection to the chain it was run for, so the indicator goes
     * on naming Face Recovery instead of falling back to the generic label for the seconds the
     * chain spends loading its model.
     */
    report?: RunProgress;
    /**
     * How far along the indicator is, in `0..1` - which is **not** {@link RunProgress.chainFraction}.
     *
     * A face recovery is two runs and one bar (see {@link DETECTION_SHARE}), so the position is
     * composed here rather than read off the report, which only ever describes the run it came
     * from. Zero for a run that has reported nothing yet.
     */
    fraction: number;
};

/**
 * Runs the current image's enhancements and answers what the canvas should draw and report.
 *
 * ```
 *   useEnhancementRun(file, crop)
 *       |                                   cleanup: cancelEnhance(run)
 *       +-- no face recovery, or its faces known ---------------------+
 *       |        |                                                    |
 *       |        enhance(identity, ops + chosen faces, proc, crop) ---+
 *       |        |                                                    |
 *       |        +-- done -> { outcome: "enhanced", identity, w, h } -+--> enhanced pane
 *       |        |           { outcome: "stopped" }  -> nothing
 *       |        +-- rejects -> notice (errors.enhanceFailed)
 *       |
 *       +-- a face recovery whose faces are not known for this framing
 *       |        |
 *       |        +-- one is already in flight for (identity, crop) -> draw it, ask nothing
 *       |        |
 *       |        detectFaces(identity, proc, crop)
 *       |        |
 *       |        +-- done -> setFaces(identity, crop, faces) --> this effect runs again and enhances
 *       |        +-- rejects -> notice (errors.faceDetectFailed), setFaces(.., []) --> and runs anyway
 *       |               (either way: nothing written once the photograph has been closed)
 *       |
 *       +-- onEnhanceProgress -> report.run === mine ? keep : discard --> bar
 * ```
 *
 * **The run path owns the detection**, which is what makes "a chain carrying a face recovery is not run
 * before its faces are known" structural rather than a guard two effects have to honour. A run started
 * early would restore nothing, would be stored as the result for an empty selection, and would have to be
 * run again the moment the faces landed - a visible flash of an unrestored result and a wasted chain.
 *
 * It is expressed as **the faces being a dependency** rather than as an `await` inside one pass: the
 * detection writes the store, the store write re-renders, and this effect runs again and finds the faces
 * it needs. So there is one path that starts a run, and the question it asks - are this photograph's faces
 * known for the framing in force? - has the same answer whether the detection just happened, happened at
 * this framing earlier, or is not needed at all. See design.md D4.
 *
 * **Detection is not asked for by anything else but Autopilot.** A photograph is not detected in because it is
 * open, selected or being looked at - only because its stack carries something that restores faces, or because an
 * analysis suggested one (`hooks/useAutopilot.ts`, which records what it finds in the same faces store). Where the
 * faces for the framing in force are already known, adding a second enhancement, changing a model or
 * changing the processor detects nothing again - and neither does any of those while the detection
 * that will answer them is still in flight, which the store cannot say because its entry appears only
 * once the answer lands.
 *
 * **A photograph that has been closed is not written.** A detection settling late is ordinarily a warm
 * cache for a photograph the window has moved off, which is why nothing cancels it; a photograph that
 * was *closed* has already had every owner told to forget it, and putting an entry back for it would
 * outlive the only thing that empties the store.
 *
 * **One ref, two run names.** `mine.current` is set to the detection's name and then to the
 * enhancement's, so the one progress subscription draws both.
 *
 * **One bar across the two of them.** Each of those runs reports its own `0..1`, so drawing them as
 * they arrive filled the bar, emptied it and filled it again - one enhancement that looked like
 * two. The detection is mapped onto the head of the range and the chain onto what it leaves (see
 * {@link DETECTION_SHARE}), and the last report is held across the handover, so the indicator moves
 * forward once and keeps naming the enhancement that asked for the faces throughout.
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
     * Whether this photograph's faces are needed at all, and whether they are known for the framing in
     * force. A miss is what asks for a detection - so a flip, a turn or a cut detects again, and a
     * framing returned to reads the answer found the first time.
     *
     * Asked of the store **only while the stack needs them**, which is what keeps a late detection
     * from disturbing a chain that no longer does: this is a dependency of the run effect below, so
     * an answer landing after the face recovery was removed would otherwise move it from `undefined`
     * to `[]` and cancel and re-ask a chain that was already running, resetting the bar with it.
     */
    const restoresFaces = operations.some((operation) => operation.family === "face_recovery");
    const faces = useImageFaces(restoresFaces ? source : undefined, crop);

    /*
     * Which of them the user has skipped, and the reason this is a dependency rather than something
     * the effect reads off the store: the set is replaced whole on a write and is the same reference
     * until one happens, so the chain re-runs exactly when a choice is applied and at no other time.
     * That is the whole of "a committed change re-runs the preview, once" - it falls out of this
     * effect instead of needing a second one. See design.md D7.
     */
    const skipped = useSkippedFaces(source);

    /*
     * The last result, beside the photograph it was made from.
     *
     * The source travels with it rather than being cleared by an effect keyed on the identity,
     * which is what makes "a result belongs to the photograph it was made from" a fact about the
     * value rather than a race between two effects: a render that has moved to another image simply
     * does not match, so there is no frame in which one image's enhancement is drawn over another's.
     */
    const [result, setResult] = useState<{ source: string; crop?: CropInfo; image: EnhancedImage }>();
    /*
     * What the bar is drawing: the last report this window kept, and the place on the indicator it
     * put it. The two are one piece of state because the position is composed from the share in
     * force when the report arrived - a report kept beside a fraction computed later could be read
     * against the wrong share across a handover.
     */
    const [indicator, setIndicator] = useState<Indicator>();

    /*
     * Set in the same turn the run is asked for, so the indicator is on screen before the backend
     * has said anything at all. `mine` cannot answer this: it is a ref precisely so the progress
     * listener can read it without re-registering, and a ref does not re-render.
     */
    const [running, setRunning] = useState(false);

    // The framing is compared by identity rather than field by field: the store replaces the whole
    // value on every write and hands back the same object until one happens, so two renders of one
    // framing are the same reference and two framings never are.
    const enhanced = result && result.source === source && result.crop === crop ? result.image : undefined;

    /*
     * The run this window is drawing, read by the progress listener - which is registered once and
     * must not close over a value that changes with every keystroke in the scale field.
     */
    const mine = useRef<string | undefined>(undefined);

    /*
     * Which part of the indicator the run named by `mine` owns. Set in the same pass that names the
     * run and read by the progress listener, so the two are always the same run's - and a ref for
     * `mine`'s own reason: the listener is registered once and must not re-register to see it.
     */
    const share = useRef<Share>(WHOLE_BAR);

    /*
     * What a detection was just run for, or `undefined` where none was: which photograph, at which
     * framing. It is what decides whether the chain a pass asks for owns the whole bar or the four
     * fifths a detection left it.
     *
     * **Named rather than a flag**, and compared against the run about to be asked for: a detection
     * started on one photograph must not give the next photograph's chain the tail of the bar, and
     * a bare "a detection happened" cannot tell the two apart.
     *
     * **Consumed** by the chain that follows, so a re-run over faces already known - a changed
     * scale, a changed processor, a face deselected - takes the whole range back rather than
     * starting a fifth of the way in for ever.
     */
    const detected = useRef<{ source: string; crop?: CropInfo } | undefined>(undefined);

    /*
     * The detection in flight, or `undefined` while there is none: which photograph it is about, at
     * which framing, and under which run name.
     *
     * The store cannot answer this. Its entry appears when the detection *lands*, so between asking
     * and landing "are this photograph's faces known?" is no and every re-render that reaches the
     * effect below - a second enhancement added, a model changed, the processor changed - would ask
     * for the same faces again. Two detectors would then run over the same pixels concurrently, both
     * transferring on a first use, and neither is a question the other has not already asked.
     *
     * A ref rather than state, for `mine`'s own reason: it is read by the effect and must not be the
     * thing that re-renders, or writing it would schedule the pass that reads it.
     */
    const detecting = useRef<{ source: string; crop?: CropInfo; run: string } | undefined>(undefined);

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

            // Mapped onto the share this run owns rather than drawn as it arrived: a detection and
            // the chain it was run for are two runs reporting `0..1` each, and one bar.
            const { base, span } = share.current;

            setIndicator({ report: incoming, fraction: base + incoming.chainFraction * span });
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
            detected.current = undefined;
            return;
        }

        /*
         * The detection, and nothing else this pass. Writing the store is what brings this effect back
         * with the faces in hand - see the diagram above - so the chain is asked for from the one place
         * it is ever asked for, with the same question already answered.
         *
         * There is no cleanup for it: a detection cannot be stopped, and an abandoned one is bounded
         * work whose result is kept where the next request for that framing is served by it. A window
         * that has moved on discards the answer by run name. See design.md D11.
         */
        if (restoresFaces && faces === undefined) {
            share.current = DETECTING;
            // Recorded whether this pass is the one that asks or one that finds the question already
            // being answered, because in both cases a detection is what the chain will follow.
            detected.current = { source, ...(crop && { crop }) };

            /*
             * The same photograph at the same framing is already being asked about, so this pass has
             * nothing to ask - a second enhancement was added, or a model or the processor changed,
             * and none of those is a question about the pixels. The run name is picked back up
             * because the top of this effect cleared it: the detection in flight is still what the
             * indicator is drawing, and it goes on being drawn across the change - which is also why
             * what it has drawn so far is left alone rather than cleared.
             */
            if (detecting.current?.source === source && detecting.current.crop === crop) {
                mine.current = detecting.current.run;
                setRunning(true);
                return;
            }

            // A sequence of its own begins here, so whatever the bar was showing goes.
            setIndicator(undefined);

            const { run, done } = detectFaces(source, processor, crop);
            mine.current = run;
            setRunning(true);
            detecting.current = { source, ...(crop && { crop }), run };

            /*
             * What a detection answered, written only while there is still a photograph to hold it
             * for.
             *
             * A late answer is ordinarily a warm cache - the store is keyed by identity and holds the
             * framing, so one landing for a photograph the window has moved off is read by nobody
             * until that photograph and framing are current again. Closing the photograph is the one
             * case where that is not true: the file store has already told every owner to forget it,
             * and a write after that would put an entry back for a photograph nobody can see and
             * nothing will close again.
             *
             * Cleared before the write, so the pass the write brings back sees no detection in flight.
             * Only where the ref still names this run: a newer detection has already replaced it, and
             * an older one settling must not say that newer one has finished.
             */
            const record = (found: Face[]) => {
                if (detecting.current?.run === run) detecting.current = undefined;

                if (!useFileStore.getState().files.some((open) => open.identity === source)) return;

                // Read off the store rather than subscribed to: what this needs is the writer, and a
                // selector would make the effect depend on a function identity zustand is free to change.
                useFacesStore.getState().setFaces(source, crop, found);
            };

            done.then(record, (error: unknown) => {
                /*
                 * Recorded as no faces rather than left unanswered, and the chain runs anyway. The
                 * alternative - refusing the chain - would mean a failed detection also cancelled an
                 * upscale the user asked for in the same list, which is a second failure caused by
                 * the first. An empty selection is a request: the recovery restores nothing and
                 * still produces the photograph. See design.md D10.
                 *
                 * The notice is raised whether or not the photograph is still open, as the failure of
                 * an enhancement run is: what it reports is that a detection this window asked for
                 * failed, and a user who has moved on from the photograph has still been told the
                 * truth about the work they asked for.
                 */
                report("detecting the faces in the image failed", error);
                toast.error(notice.current("errors.faceDetectFailed"));

                record([]);
            });

            return;
        }

        /*
         * The chosen faces, built **inside** the effect rather than in the render body: `enabledFaces`
         * answers a fresh array whenever anything is skipped, and listing that among the dependencies
         * below would cancel the run in flight and start another on every render.
         *
         * `faces` is only ever absent here for a stack that carries no face recovery, in which case
         * nothing reads the value put in.
         */
        const chosen = enabledFaces(faces ?? [], skipped);

        /*
         * Whether a detection preceded this chain, **consumed** as it is read: only the chain the
         * detection was run for starts where that detection left off, and every later run over the
         * same faces owns the bar in full. Matched on the photograph and the framing, so a detection
         * the window has moved off does not hand its tail to whatever runs next.
         */
        const following = detected.current?.source === source && detected.current.crop === crop;
        detected.current = undefined;
        share.current = following ? AFTER_DETECTION : WHOLE_BAR;

        /*
         * What the detection drew is held across the handover rather than cleared. The chain spends
         * its first seconds loading a model and reports nothing while it does, so clearing here put
         * the bar back to no progress under the generic label - the empty half of the second sweep
         * this split exists to remove. A chain nothing preceded has nothing to hold.
         */
        if (!following) setIndicator(undefined);

        const { run, done } = enhance(source, withFaces(operations, chosen), processor, crop);
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

        done.then(
            (outcome) => {
                ended();

                if (outcome.outcome === "enhanced") {
                    setResult({
                        source,
                        ...(crop && { crop }),
                        image: { identity: outcome.identity, width: outcome.width, height: outcome.height },
                    });
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
        // The faces are one of these because they are what the pass above is waiting for: the detection
        // writes them, this runs again, and that second pass is the one that enhances. They change only
        // when a detection lands or the photograph is closed, so nothing else re-runs on their account.
        //
        // The choice made among them is one of these for the same shape of reason, and it is the set
        // rather than the filtered array deliberately: the set changes only when a choice is applied,
        // and the array it produces would be new on every render.
    }, [source, operations, processor, crop, restoresFaces, faces, skipped]);

    return {
        running,
        // Zero for a run that has reported nothing yet, which is what the bar draws while a model
        // loads - the one honest thing to say about a run that has not spoken.
        fraction: indicator?.fraction ?? 0,
        ...(enhanced && { enhanced }),
        ...(indicator && { report: indicator.report }),
    };
};
