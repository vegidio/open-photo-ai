import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ExportRow } from "@/features/export/ExportRow";
import { useExportBatchStore } from "@/stores/exportBatch";

/** The keys that scroll a list, which stop it following as a wheel does. */
const SCROLL_KEYS = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "]);

/**
 * The queue: one row per photograph picked when the dialog opened, in the order they are exported.
 *
 * The order is the batch store's snapshot, grouped so photographs needing the same models are next to each other,
 * which the header says. It does not change while the dialog is open.
 *
 * **It follows the row being worked on** until the user scrolls it by hand - a wheel, a touch, a grab of the scrollbar
 * or a scroll key - and then leaves the list where they put it until the next run starts.
 */
export const ExportQueue = () => {
    const { t } = useTranslation();
    const queue = useExportBatchStore((state) => state.queue);
    const running = useExportBatchStore((state) => state.phase === "running");
    const [following, setFollowing] = useState(true);
    const listRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (running) setFollowing(true);
    }, [running]);

    // Listened for on the list rather than as props: they watch how it is scrolled, and make it no more interactive.
    useEffect(() => {
        const list = listRef.current;
        if (!list) return;

        const stop = () => setFollowing(false);
        // Only a press on the list itself, which is its scrollbar; a press on a row is a click on the row.
        const grabbed = (event: PointerEvent) => event.target === list && stop();
        const keyed = (event: KeyboardEvent) => SCROLL_KEYS.has(event.key) && stop();

        list.addEventListener("wheel", stop, { passive: true });
        list.addEventListener("touchmove", stop, { passive: true });
        list.addEventListener("pointerdown", grabbed);
        list.addEventListener("keydown", keyed);

        return () => {
            list.removeEventListener("wheel", stop);
            list.removeEventListener("touchmove", stop);
            list.removeEventListener("pointerdown", grabbed);
            list.removeEventListener("keydown", keyed);
        };
    }, []);

    return (
        <div className="flex min-w-0 flex-1 flex-col gap-3 p-4" data-slot="export-queue">
            <div className="flex items-baseline gap-2.5">
                <span className="font-semibold text-[13px]">{t("export.queue.title", { count: queue.length })}</span>
                <span className="text-foreground-faint text-xs">{t("export.queue.pipelineOrder")}</span>
            </div>

            {/* Scrolled rather than clipped as the mockup's four rows are: a batch can be forty photographs. */}
            <div ref={listRef} className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto scrollbar-thin">
                {queue.map((path) => (
                    <ExportRow key={path} path={path} follow={following} />
                ))}
            </div>
        </div>
    );
};
