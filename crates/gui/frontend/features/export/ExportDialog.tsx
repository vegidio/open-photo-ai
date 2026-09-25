import { useEffect, useLayoutEffect } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { Progress } from "@/components/ui/progress";
import { routeProgress } from "@/features/export/batch";
import { ExportQueue } from "@/features/export/ExportQueue";
import { ExportSettingsPanel } from "@/features/export/ExportSettingsPanel";
import { onExportProgress } from "@/ipc/export";
import { groupByChain } from "@/lib/export";
import { report } from "@/lib/report";
import { useEnhancementStore } from "@/stores/enhancements";
import { batchSummary, overallFraction, useExportBatchStore } from "@/stores/exportBatch";
import { useFileStore } from "@/stores/files";

/** A progress subscription that could not be made, which leaves the bars still while the files are still written. */
const subscribeFailed = (error: unknown) => report("subscribing to the export's progress failed", error);

/** The title bar's summary of the batch, in whichever phase it is. */
const Summary = () => {
    const { t } = useTranslation();
    const { phase, total, written, failed, remaining } = useExportBatchStore(useShallow(batchSummary));

    const text =
        phase === "idle"
            ? t("export.summary.ready", { count: total })
            : phase === "running"
              ? t("export.summary.running", { count: total, written, remaining })
              : failed > 0
                ? t("export.summary.endedFailed", { count: total, written, failed })
                : t("export.summary.ended", { count: total, written });

    return <span className="min-w-0 flex-1 truncate text-[13px] text-foreground-dim">{text}</span>;
};

/**
 * Screens 15 to 15e: the picked photographs, queued, beside the settings that decide each file, with the batch run
 * from its buttons.
 *
 * **Mounted while open.** The queue is snapshotted as it mounts - the picked records in drawer order, grouped by
 * chain - and does not change while it is open. One progress subscription is held for its life.
 *
 * **It cannot be dismissed while the batch runs**: no close box, and Escape is refused. A click outside never closes
 * it. The one way out of a running batch is Abort. Unmounting aborts anyway, for a window that goes away under it.
 */
export const ExportDialog = ({ onClose }: { onClose: () => void }) => {
    const { t } = useTranslation();
    const phase = useExportBatchStore((state) => state.phase);
    const overall = useExportBatchStore(overallFraction);

    // Before paint, so the first frame drawn is this dialog's queue rather than the last one's.
    useLayoutEffect(() => {
        const { files, selectedPaths } = useFileStore.getState();
        const { enhancements } = useEnhancementStore.getState();
        const picked = files.filter((file) => selectedPaths.has(file.path)).map((file) => file.path);

        useExportBatchStore.getState().open(groupByChain(picked, (path) => enhancements.get(path)));

        return () => useExportBatchStore.getState().abort();
    }, []);

    useEffect(() => {
        // `listen` answers its remover asynchronously, so an unmount that beats it removes the listener on arrival.
        let unlisten: (() => void) | undefined;
        let gone = false;

        onExportProgress(routeProgress).then((remove) => {
            if (gone) remove();
            else unlisten = remove;
        }, subscribeFailed);

        return () => {
            gone = true;
            unlisten?.();
        };
    }, []);

    const running = phase === "running";

    return (
        <Dialog
            open
            // Escape and the close box both arrive here, and neither closes a running batch.
            onOpenChange={(next) => !next && useExportBatchStore.getState().phase !== "running" && onClose()}
        >
            <DialogContent
                showCloseButton={false}
                aria-describedby={undefined}
                onInteractOutside={(event) => event.preventDefault()}
                // The design's 1120 x 672, capped by a window smaller than that. `max-w-none` twice for the reason
                // `SettingsDialog` gives.
                className="flex h-168 max-h-[calc(100vh-2rem)] w-280 max-w-[calc(100vw-2rem)] flex-col gap-0 overflow-hidden rounded-xl border border-border bg-card p-0 sm:max-w-[calc(100vw-2rem)]"
            >
                <DialogTitleBar title={t("export.title")} showCloseButton={!running}>
                    <Summary />
                </DialogTitleBar>

                <Progress
                    value={overall * 100}
                    aria-label={t("export.title")}
                    className="h-0.75 flex-none rounded-none bg-secondary"
                />

                <div className="flex min-h-0 flex-1">
                    <ExportQueue />
                    <ExportSettingsPanel onClose={onClose} />
                </div>
            </DialogContent>
        </Dialog>
    );
};
