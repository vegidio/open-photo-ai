import { useOnce } from "@/hooks/useOnce";
import { type ExportFormats, exportFormats } from "@/ipc/export";

/**
 * Every format an export can be written in and what each can do, once it has arrived - `undefined` for the render
 * before it has. See `useOnce`.
 */
export const useExportFormats = (): ExportFormats | undefined => useOnce(exportFormats);
