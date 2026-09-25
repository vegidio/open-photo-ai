import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { openImages } from "@/ipc/images";
import { formatsOf } from "@/lib/events";
import { track } from "@/lib/faro";
import { report } from "@/lib/report";
import { useFileStore } from "@/stores/files";

/**
 * Opens the platform's picker and adds whatever the user chose to the window's open images.
 *
 * The two controls that add images - the empty canvas's Browse images and the drawer's Add images -
 * are one behaviour drawn in two places, so they call one hook. The reference reaches the same
 * conclusion in `useFileManager`, and for the reason its comment gives: the picker's wording is
 * passed from the frontend because the backend has no catalogue of its own, and doing that in one
 * place is what keeps the two buttons from drifting apart on it.
 *
 * **A dismissal is not a failure.** The command resolves with no files for a picker the user closed
 * and for a picker the platform would not open, which it cannot tell apart; both mean the user added
 * no images, so the store is handed an empty batch that changes nothing and the user is told nothing.
 * Reporting it would be a notice about them changing their mind.
 *
 * A genuine rejection - the application closing while the chosen files were being described - is
 * logged and nothing else. Rust has already written it to the log file, and there is no screen for it
 * in an application that is on its way out.
 *
 * `source` names the control for the `files_added` event: the drawer's Add images browses, the empty
 * canvas's control is its own. A dismissal adds nothing, and sends nothing.
 */
export const useOpenImages = (source: "browse" | "empty") => {
    const { t } = useTranslation();
    const addFiles = useFileStore((state) => state.addFiles);

    return useCallback(() => {
        // Translated here rather than in Rust: the extension list is derived from what the decoder
        // opens and stays on that side, the two words are the catalogue's and stay on this one.
        openImages(t("dialogs.native.selectImage"), t("dialogs.native.imagesFilter")).then(
            (files) => {
                addFiles(files);

                if (files.length > 0) {
                    track("files_added", {
                        count: files.length,
                        source,
                        formats: formatsOf(files.map((file) => file.path)),
                    });
                }
            },
            (error: unknown) => report("Failed to open the file picker", error),
        );
    }, [addFiles, source, t]);
};
