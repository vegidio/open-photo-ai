import { useEffect, useState } from "react";
import { type ExportFormats, exportFormats } from "@/ipc/export";

/**
 * Every format an export can be written in and what each can do, once it has arrived - `undefined` for the render
 * before it has.
 *
 * `useCatalogue`'s shape, for its reasons: an `invoke` answered once and kept for the life of the process, so every
 * consumer mounting at once is one call. No error branch either: a rejection here means the IPC bridge is broken.
 */
export const useExportFormats = (): ExportFormats | undefined => {
    const [formats, setFormats] = useState<ExportFormats>();

    useEffect(() => {
        let live = true;

        exportFormats().then((answered) => live && setFormats(answered));

        return () => {
            live = false;
        };
    }, []);

    return formats;
};
