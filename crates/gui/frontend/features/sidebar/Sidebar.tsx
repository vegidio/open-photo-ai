import { useState } from "react";
import { Upload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { AddEnhancement } from "@/features/enhancements/AddEnhancement";
import { EnhancementList } from "@/features/enhancements/EnhancementList";
import { ExportDialog } from "@/features/export/ExportDialog";
import { SidebarMiniature } from "@/features/sidebar/SidebarMiniature";
import { useAutopilot } from "@/hooks/useAutopilot";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFileStore } from "@/stores/files";

/**
 * The column down the right: what the current image looks like, what is being done to it, and the way
 * out to a file.
 *
 * **Export acts on the picked images**, not on the current one: it is available while at least one is picked, and
 * reads in the singular or the plural by how many. Autopilot is live whether or not one is open.
 *
 * **Autopilot runs from here**: the trigger is mounted once, beside the switch that governs it and the list its
 * answers land in.
 */
export const Sidebar = () => {
    const { t } = useTranslation();
    useAutopilot();
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const toggle = useEnhancementStore((state) => state.toggle);
    const picked = useFileStore((state) => state.selectedPaths.size);
    const [exporting, setExporting] = useState(false);

    return (
        <aside className="flex w-64 flex-none flex-col border-l border-border bg-card">
            <SidebarMiniature />

            <div className="flex flex-col gap-5 border-b border-border bg-background p-6">
                {/*
                 * Autopilot is live deliberately: it is a standing preference about what should happen
                 * when an image is loaded, not an action on the one that is open.
                 *
                 * `htmlFor` rather than wrapping the switch: Radix renders it as a
                 * `<button role="switch">`, so a label around it reads to a linter as a label around
                 * nothing. `for` on a labelable element is the same association and says so outright.
                 */}
                <label htmlFor="autopilot" className="flex items-center justify-between">
                    <span className="text-[13px] font-semibold text-success-bright">{t("sidebar.autopilot")}</span>
                    <Switch
                        id="autopilot"
                        checked={autopilot}
                        onCheckedChange={toggle}
                        /*
                         * Both states, because the generated primitive colours only the checked one
                         * and leaves the track transparent otherwise - which on this block's
                         * `--background` makes an off switch invisible rather than merely subtle.
                         * The two values are the design's: `--success` on, `--input` off.
                         */
                        className="data-[state=unchecked]:bg-input data-[state=checked]:bg-success"
                    />
                </label>

                {/*
                 * Gated on there being an image to add one to - its own question, asked where it is
                 * answered rather than threaded down.
                 */}
                <AddEnhancement />
            </div>

            <EnhancementList />

            <div className="flex-1" />

            {/*
             * Gated on the selection rather than on an image being open: nothing can be picked with
             * nothing open, so "unavailable while none is open" still holds by construction.
             */}
            <Button
                variant="secondary"
                disabled={picked === 0}
                onClick={() => setExporting(true)}
                className="h-12 w-full flex-none rounded-none border-t border-border font-normal disabled:opacity-35"
            >
                <Upload className="size-4.5 text-primary" />
                {/* The singular with nothing picked, as the empty window's design draws it. */}
                {t("sidebar.exportImage", { count: Math.max(picked, 1) })}
            </Button>

            {/* Mounted while open, which is what snapshots its queue as it opens. */}
            {exporting && <ExportDialog onClose={() => setExporting(false)} />}
        </aside>
    );
};
