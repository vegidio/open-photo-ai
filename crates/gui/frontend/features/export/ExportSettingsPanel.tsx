import { type ReactNode, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { runBatch } from "@/features/export/batch";
import { pickDirectory } from "@/ipc/export";
import { useExportFormats } from "@/hooks/useExportFormats";
import { FORMAT_CHOICES, type FormatChoice, queueQualityFor } from "@/lib/export";
import { cn } from "@/lib/utils";
import { useExportBatchStore } from "@/stores/exportBatch";
import { exportSettingsData, useExportSettingsStore } from "@/stores/exportSettings";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";

/** What each format reads as in the chooser. JPEG is `JPG`, as the reference and the design spell it. */
const FORMAT_LABELS: Record<Exclude<FormatChoice, "preserve">, string> = {
    avif: "AVIF",
    bmp: "BMP",
    gif: "GIF",
    heic: "HEIC",
    jpeg: "JPG",
    png: "PNG",
    tiff: "TIFF",
    webp: "WEBP",
};

// The Save-to chooser's three values. `chosen` is the reference's hidden item: never offered, it is what the chooser
// shows while a browsed folder is in force, with the folder's path as its text. `browse` is never the value - choosing
// it opens the picker and leaves the value where it was.
type LocationValue = "chosen" | "original" | "browse";

/** One bordered group of the settings column. */
const Card = ({ className, children }: { className?: string; children: ReactNode }) => (
    <div className={cn("flex flex-col gap-2 rounded-lg border border-border p-3", className)}>{children}</div>
);

const Caption = ({ children }: { children: ReactNode }) => (
    <span className="text-foreground-dim text-xs">{children}</span>
);

const FIELD = "h-8 bg-background px-2.5 text-[13px] md:text-[13px]";

/**
 * The export settings column: the filename's affixes, where to save, whether to overwrite, the format and the
 * quality, with the batch's two buttons under them.
 *
 * **Every setting but the quality is written as it changes**, so the queue's names follow it while idle. **The
 * quality is a draft**, seeded from Settings once when the dialog opens - through `getState()` so the Settings
 * dialog cannot pull a half-made edit out from under it - and committed only by Save. Closing discards it.
 *
 * **The whole column is unavailable while a batch runs**, where the reference leaves it live: the batch writes the
 * names it was saved with, and the rows would otherwise show others. See design.md D8.
 */
export const ExportSettingsPanel = ({ onClose }: { onClose: () => void }) => {
    const { t } = useTranslation();
    const { prefix, suffix, location, overwrite, format, setPrefix, setSuffix, setLocation, setOverwrite, setFormat } =
        useExportSettingsStore();
    const phase = useExportBatchStore((state) => state.phase);
    const running = phase === "running";

    // The queue's records, for the one lossy format a slider could stand for. Read off the file list by the queue's
    // paths; the queue is fixed while the dialog is open, and the records do not change under it.
    const queue = useExportBatchStore((state) => state.queue);
    const files = useFileStore((state) => state.files);
    const records = useMemo(() => {
        const queued = new Set(queue);

        return files.filter((file) => queued.has(file.path));
    }, [files, queue]);
    const formats = useExportFormats();
    const queueQuality = formats && queueQualityFor(records, format, formats);

    const [draft, setDraft] = useState(() => ({ ...useSettingsStore.getState().quality }));

    /** What the slider shows for the queue's format: the draft's value, or the format's published default. */
    const shown = ({ format, range }: NonNullable<typeof queueQuality>) => draft[format] ?? range.default;

    const choose = async (value: string) => {
        if (value === "original") return setLocation(undefined);
        if (value !== "browse") return;

        // A dismissal keeps the previous choice, the original directory included.
        const folder = await pickDirectory(t("dialogs.native.selectDirectory"));
        if (folder !== undefined) setLocation(folder);
    };

    const save = () => {
        // Only the format the slider stands for: a draft left on another format, by moving the slider and then
        // changing the format, was never shown as what this batch writes. Through Settings' own save path, and then
        // read back, so what is written is exactly the value kept, clamping included.
        if (queueQuality) {
            const { quality, apply } = useSettingsStore.getState();
            apply({ quality: { ...quality, [queueQuality.format]: shown(queueQuality) } });
        }

        void runBatch(exportSettingsData(useExportSettingsStore.getState()), useSettingsStore.getState().quality);
    };

    const locationValue: LocationValue = location === undefined ? "original" : "chosen";

    return (
        <div
            // The design's one surface between `--background` and `--card`, drawn nowhere else, so it has no token.
            className="flex w-85 flex-none flex-col gap-3.5 border-l border-border bg-[#141417] p-4"
            data-slot="export-settings"
        >
            <span className="font-semibold text-[13px]">{t("export.settings.title")}</span>

            <div className="flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto scrollbar-thin">
                <Card>
                    <Caption>{t("export.settings.filename.title")}</Caption>
                    <div className="grid grid-cols-2 gap-2">
                        <Input
                            aria-label={t("export.settings.filename.prefix")}
                            placeholder={t("export.settings.filename.prefix")}
                            value={prefix}
                            disabled={running}
                            onChange={(event) => setPrefix(event.target.value)}
                            className={FIELD}
                        />
                        <Input
                            aria-label={t("export.settings.filename.suffix")}
                            placeholder={t("export.settings.filename.suffix")}
                            value={suffix}
                            disabled={running}
                            onChange={(event) => setSuffix(event.target.value)}
                            className={FIELD}
                        />
                    </div>
                </Card>

                <Card>
                    <Caption>{t("export.settings.location.title")}</Caption>
                    <Select value={locationValue} onValueChange={(value) => void choose(value)} disabled={running}>
                        <SelectTrigger
                            aria-label={t("export.settings.location.title")}
                            className={cn(FIELD, "w-full min-w-0")}
                        >
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent position="popper">
                            {location !== undefined && (
                                <SelectItem value="chosen" className="hidden">
                                    {location}
                                </SelectItem>
                            )}
                            <SelectItem value="original">{t("export.settings.location.original")}</SelectItem>
                            <SelectItem value="browse">{t("export.settings.location.browse")}</SelectItem>
                        </SelectContent>
                    </Select>

                    <label htmlFor="export-overwrite" className="mt-0.5 flex items-center justify-between">
                        <span className="text-[13px]">{t("export.settings.filename.allowOverwrite")}</span>
                        <Switch
                            id="export-overwrite"
                            checked={overwrite}
                            disabled={running}
                            onCheckedChange={setOverwrite}
                            // Both states, for the reason the sidebar's Autopilot switch gives.
                            className="data-[state=unchecked]:bg-input data-[state=checked]:bg-primary"
                        />
                    </label>
                    <p
                        data-slot="overwrite-note"
                        className={cn(
                            "m-0 text-pretty text-xs/normal",
                            overwrite ? "text-warning" : "text-foreground-dim",
                        )}
                    >
                        {overwrite
                            ? t("export.settings.filename.sameLocationOverwrite")
                            : t("export.settings.filename.sameLocationRename")}
                    </p>
                </Card>

                <Card className="gap-2.5">
                    <div className="flex items-center justify-between gap-3">
                        <Caption>{t("export.settings.format.title")}</Caption>
                        <Select
                            value={format}
                            onValueChange={(value) => setFormat(value as FormatChoice)}
                            disabled={running}
                        >
                            <SelectTrigger aria-label={t("export.settings.format.title")} className={cn(FIELD, "w-35")}>
                                <SelectValue />
                            </SelectTrigger>
                            <SelectContent position="popper">
                                {FORMAT_CHOICES.map((choice) => (
                                    <SelectItem key={choice} value={choice}>
                                        {choice === "preserve"
                                            ? t("export.settings.format.preserve")
                                            : FORMAT_LABELS[choice]}
                                    </SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>

                    {/* Only where one lossy format stands for the whole queue; see `queueQualityFor`. */}
                    {queueQuality && (
                        <div className="flex items-center justify-between gap-3" data-slot="export-quality">
                            <Caption>{t("export.settings.quality.title")}</Caption>
                            <div className="flex w-35 items-center gap-2.5">
                                <Slider
                                    value={[shown(queueQuality)]}
                                    min={queueQuality.range.min}
                                    max={queueQuality.range.max}
                                    step={1}
                                    disabled={running}
                                    thumbLabel={t("export.settings.quality.title")}
                                    className="flex-1 **:data-[slot=slider-track]:bg-input"
                                    onValueChange={([next]) =>
                                        next !== undefined && setDraft({ ...draft, [queueQuality.format]: next })
                                    }
                                />
                                <span className="w-6 text-right font-mono text-[13px] text-muted-foreground">
                                    {shown(queueQuality)}
                                </span>
                            </div>
                        </div>
                    )}
                </Card>
            </div>

            <div className="flex flex-none gap-2.5">
                {phase === "idle" && (
                    <>
                        <Button variant="secondary" className="h-9 flex-1" onClick={onClose}>
                            {t("common.cancel")}
                        </Button>
                        <Button className="h-9 flex-1" onClick={save}>
                            {t("common.save")}
                        </Button>
                    </>
                )}
                {running && (
                    <>
                        <Button
                            variant="secondary"
                            className="h-9 flex-1"
                            onClick={() => useExportBatchStore.getState().abort()}
                        >
                            {t("export.settings.abort")}
                        </Button>
                        <Button className="h-9 flex-1 disabled:opacity-50" disabled>
                            {t("common.save")}
                        </Button>
                    </>
                )}
                {phase === "ended" && (
                    <>
                        <Button variant="secondary" className="h-9 flex-1" onClick={onClose}>
                            {t("common.close")}
                        </Button>
                        <Button
                            variant="secondary"
                            className="h-9 flex-1"
                            onClick={() => useExportBatchStore.getState().reset()}
                        >
                            {t("export.settings.exportAgain")}
                        </Button>
                    </>
                )}
            </div>
        </div>
    );
};
