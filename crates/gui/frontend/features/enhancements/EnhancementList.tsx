import { useTranslation } from "react-i18next";
import { familyEntry, useCatalogue } from "@/hooks/useCatalogue";
import { ENHANCEMENTS } from "@/lib/enhancements";
import { track } from "@/lib/faro";
import { useAnalysing } from "@/stores/autopilot";
import { useEnhancementStore, useFileEnhancements } from "@/stores/enhancements";
import { useCurrentFile } from "@/stores/files";
import { EnhancementRow } from "./EnhancementRow";

/**
 * The one row drawn while the current photograph is being analysed: a spinning ring and "Analysing image...".
 *
 * Design screen 04's metrics. `role="status"` so the announcement reaches a screen reader, which is the only way
 * a user who cannot see the ring learns why the list is not there.
 */
const AnalysingRow = () => {
    const { t } = useTranslation();

    return (
        <div role="status" className="flex items-center gap-4 py-3 pr-4 pl-10" data-slot="autopilot-analysing">
            <span
                aria-hidden
                className="size-5.5 flex-none animate-spin rounded-full border-2 border-border border-t-success-bright border-r-activity"
            />
            <span className="text-sm font-bold text-primary">{t("sidebar.analysing")}</span>
        </div>
    );
};

/**
 * What the current image is set to have done to it, a row per enhancement, in the order the
 * enhancements are applied - or, while it is being analysed by Autopilot, the analysing row in their place.
 */
export const EnhancementList = () => {
    const file = useCurrentFile();
    const families = useCatalogue();
    // The order is the store's: it puts a stack back into pipeline order as each enhancement is added,
    // so the list drawn here and the chain that is sent are the same array rather than two orderings
    // that could disagree.
    const stack = useFileEnhancements(file?.path);
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);
    const analysing = useAnalysing(file?.path);

    if (!file) return;

    // Before the empty-stack check, so a photograph with nothing yet still shows it. It hides a list the user
    // added to by hand during the analysis, as design screen 04 and the reference both do; the row lasts for one
    // analysis, and the hand-added entry survives the merge.
    if (analysing) return <AnalysingRow />;

    if (stack.length === 0) return;

    return (
        <div className="flex flex-col py-2" data-slot="enhancement-list">
            {stack.map((operation) => {
                const enhancement = ENHANCEMENTS.find((entry) => entry.family === operation.family);
                // A family `ENHANCEMENTS` does not name draws nothing rather than an unnamed row: it
                // cannot arrive from the menu, and a row with no icon and no name says less than no
                // row at all.
                if (!enhancement) return null;

                return (
                    <EnhancementRow
                        // The family, which the store's one-entry-per-family rule makes unique within
                        // a stack. The reference keys its rows on an operation id, which exists there
                        // for persisted state; nothing here is persisted.
                        key={operation.family}
                        enhancement={enhancement}
                        operation={operation}
                        entry={familyEntry(families, operation.family)}
                        {...(file.identity && { identity: file.identity })}
                        onRemove={() => {
                            removeEnhancement(file.path, operation.family);
                            track("enhancement_removed", { family: operation.family });
                        }}
                        onChange={(changed) => replaceEnhancement(file.path, changed)}
                    />
                );
            })}
        </div>
    );
};
