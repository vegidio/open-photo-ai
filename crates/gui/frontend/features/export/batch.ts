import { awaitAnalysis } from "@/hooks/useAutopilot";
import i18n from "@/i18n";
import { type ExportProgress, exportImage } from "@/ipc/export";
import { detectFaces, type Face } from "@/ipc/faces";
import { describeExportError, destinationFor, type ExportFailure, formatFor, qualityFormatFor } from "@/lib/export";
import { enabledFaces, withFaces } from "@/lib/faces";
import { track } from "@/lib/faro";
import { report } from "@/lib/report";
import { useCropStore } from "@/stores/crop";
import { useEnhancementStore } from "@/stores/enhancements";
import { batchSummary, useExportBatchStore } from "@/stores/exportBatch";
import type { ExportSettingsData } from "@/stores/exportSettings";
import { useFacesStore } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { DEFAULT_QUALITY, type QualityFormat, useSettingsStore } from "@/stores/settings";

const batch = () => useExportBatchStore.getState();

/** Whether the loop goes on to the next file, or stops where it is. */
type Next = "next" | "stop";

/**
 * Exports one queued file, moving its row through its stages, and answers whether the batch goes on.
 *
 * Everything it runs is read at the moment its turn comes, not when the batch started: the stack, which an Autopilot
 * analysis may just have written, the crop, the faces and the processor. `settings` and `quality` are what Save was
 * pressed with. See design.md D1 and D2.
 */
const exportOne = async (
    path: string,
    settings: ExportSettingsData,
    quality: Record<QualityFormat, number>,
): Promise<Next> => {
    const { setStage, setCurrent } = batch();
    const file = useFileStore.getState().files.find((open) => open.path === path);
    const destination = file ? destinationFor(file, settings, settings.format) : path;

    const fail = (failure: ExportFailure) =>
        setStage(path, { stage: "failed", reason: describeExportError(i18n.t, failure, destination) });

    // Put back as not reached, where Abort was pressed during the await just ended: nothing was written for it.
    const halted = () => {
        if (!batch().aborted) return false;

        setStage(path, { stage: "queued" });
        return true;
    };

    // A record with no identity is one whose bytes could not be read: nothing can address its pixels to export.
    if (!file || file.identity === undefined) {
        fail({ cause: "unreadable" });
        return "next";
    }

    const { identity } = file;
    const crop = useCropStore.getState().crops.get(identity);
    const stackOf = () => useEnhancementStore.getState().enhancements.get(path);

    setCurrent({ path });

    if (useEnhancementStore.getState().autopilot && stackOf() === undefined) {
        setStage(path, { stage: "analysing" });
        setCurrent({ path, analysing: true });

        const outcome = await awaitAnalysis(file, crop);
        setCurrent({ path });
        if (halted()) return "stop";

        // A stop nobody here asked for - there is none while the dialog covers the window - is reported as a failed
        // analysis rather than exported with a list it never finished.
        if (outcome !== "added") {
            fail({ cause: "analysis", ...(typeof outcome === "object" && { error: outcome.failed }) });
            return "next";
        }
    }

    const operations = stackOf() ?? [];
    const { processor } = useSettingsStore.getState();

    let faces: Face[] = [];
    if (operations.some((operation) => operation.family === "face_recovery")) {
        const known = useFacesStore.getState().faces.get(identity);

        // By reference, as `useImageFaces` compares the framing: faces found at another framing are not these.
        if (known && known.crop === crop) {
            faces = known.faces;
        } else {
            setStage(path, { stage: "analysing" });

            let found: Face[];
            try {
                found = await detectFaces(identity, processor, crop).done;
            } catch (error) {
                // Recorded as no faces and exported anyway, as the canvas's run does: refusing the file would make a
                // failed detection cost the rest of its enhancements too.
                report("detecting the faces for an export failed", error);
                found = [];
            }

            // What an aborted detection found is discarded; it has no stop, so this is where the abort lands.
            if (halted()) return "stop";

            useFacesStore.getState().setFaces(identity, crop, found);
            faces = found;
        }
    }

    // Read after `setFaces`, which decides the default among faces nothing has decided about yet.
    const chosen = enabledFaces(faces, useFacesStore.getState().skipped.get(identity));

    // A lossless format takes no quality; the number is sent only because the wire takes one.
    const qualityFormat = qualityFormatFor(file, settings.format);
    const request = {
        destination,
        format: formatFor(file, settings.format),
        quality: qualityFormat ? quality[qualityFormat] : DEFAULT_QUALITY.jpeg,
        overwrite: settings.overwrite,
    };

    const { run, done } = exportImage(identity, withFaces(operations, chosen), processor, request, crop);
    setCurrent({ path, run });
    setStage(path, { stage: "enhancing", fraction: 0 });

    try {
        const answer = await done;
        // Cleared before the row is written, so a report still on its way for this run cannot take a Done back.
        setCurrent(undefined);

        // Done even after an Abort: a stop that arrived once the file was being written let the write finish.
        if (answer.outcome === "exported") {
            setStage(path, { stage: "done", path: answer.path, bytes: answer.bytes });
            return batch().aborted ? "stop" : "next";
        }

        // Only Abort stops an export, and the stopped file wrote nothing.
        setStage(path, { stage: "queued" });
        return "stop";
    } catch (error) {
        setCurrent(undefined);
        fail({ cause: "export", error });

        return batch().aborted ? "stop" : "next";
    }
};

/**
 * Runs the queue one file at a time, in queue order, until it ends, whether it finished, failed in part or was
 * aborted.
 *
 * **A failed file does not stop the batch**, where the reference's stopped at the first. **Abort does**: the file in
 * progress goes back to Queued with nothing written, and no further file starts. See design.md D2.
 *
 * A plain function rather than a hook, as `analyse` is: it outlives any one render and awaits across steps, reading
 * every store through `getState()` at the step that needs it.
 */
export const runBatch = async (settings: ExportSettingsData, quality: Record<QualityFormat, number>) => {
    const { queue, start } = batch();
    const fileCount = queue.length;

    track("export_started", {
        file_count: fileCount,
        format: settings.format,
        processor: useSettingsStore.getState().processor,
    });
    const began = Date.now();
    start();

    /** Whether the queue was worked through to its end, rather than aborted or stopped. */
    const ranToEnd = async () => {
        for (const path of queue) {
            if (batch().aborted) return false;
            if ((await exportOne(path, settings, quality)) === "stop") return false;
        }

        return true;
    };

    let completed = false;

    try {
        completed = await ranToEnd();
    } finally {
        batch().finish();

        // Whether people wait for a batch or abort it. Failed files are not counted here: each is
        // already the backend's record.
        track("export_finished", {
            file_count: fileCount,
            exported: batchSummary(batch()).written,
            completed,
            duration_ms: Date.now() - began,
        });
    }
};

/**
 * Moves the row being exported on by one progress report: its bar while the chain runs, then Writing.
 *
 * A report for any other run is dropped - a canvas run's never arrives here, but an export's that trails its own
 * answer does, and must not move a row the batch has moved on from.
 */
export const routeProgress = (report: ExportProgress) => {
    const { current, setStage } = batch();
    if (current?.run === undefined || current.run !== report.run) return;

    setStage(
        current.path,
        report.phase === "writing" ? { stage: "writing" } : { stage: "enhancing", fraction: report.chainFraction },
    );
};
