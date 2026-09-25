import type { TFunction } from "i18next";
import { toast } from "sonner";
import { revealLog } from "@/ipc/logs";
import { report } from "@/lib/report";

/**
 * Show the user their log file, selected in the platform's file manager, or say why it could not be.
 *
 * The one Show logs, behind both the settings dialog's button and the recovery screen's. One command:
 * `revealLog` resolves the path in Rust, so nothing this side sends decides which file is shown and
 * there is no path to resolve here first.
 *
 * **A failure is reported rather than passed over** - both buttons exist to help someone file a report
 * about something else, and a button that quietly does nothing leaves them with neither the file nor a
 * reason. The reason in full goes to the log rather than into the toast: `errors.showLogsFailed`
 * already tells the user where to look in words, in the language the application is running in, and a
 * toast is a notice that fades before anyone copies a path out of it.
 *
 * Here rather than beside `revealLog`, because `ipc/` sits beneath the reporting and the toasts.
 */
export const showLogs = async (t: TFunction) => {
    try {
        await revealLog();
    } catch (error) {
        report("showing the log file failed", error);
        toast.error(t("errors.showLogsFailed"));
    }
};
