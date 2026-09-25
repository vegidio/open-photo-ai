import { useEffect, useRef, useState } from "react";
import { ArrowRight, CircleAlert, Folder } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Progress } from "@/components/ui/progress";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { showInLabel } from "@/features/files/FileOptionsMenu";
import { useExportFormats } from "@/hooks/useExportFormats";
import { framedDimensions } from "@/ipc/crop";
import { revealExport } from "@/ipc/export";
import { renditionFor } from "@/ipc/images";
import { upscaleFactor } from "@/lib/enhancements";
import { exportNameFor, extensionFor } from "@/lib/export";
import { report } from "@/lib/report";
import { cn, formatBytes, formatDimensions } from "@/lib/utils";
import { useImageCrop } from "@/stores/crop";
import { useFileEnhancements } from "@/stores/enhancements";
import { type Row, useExportBatchStore } from "@/stores/exportBatch";
import { useExportSettingsStore } from "@/stores/exportSettings";
import { useFileByPath } from "@/stores/files";

// The design's thumbnail is 48px, drawn at twice that so it stays sharp on a Retina display.
const ROW_THUMBNAIL_BOUND = 96;

const QUEUED: Row = { stage: "queued" };

/** The pill for every stage but Failed, which draws its own. */
const STAGE_PILL: Record<Exclude<Row["stage"], "failed">, string> = {
    queued: "bg-secondary text-foreground-dim",
    analysing: "bg-primary/14 text-primary",
    enhancing: "bg-primary/14 text-primary",
    writing: "bg-primary/14 text-primary",
    done: "bg-success-bright/12 text-success-bright",
};

/**
 * The Failed pill: hovering it shows why, in a red tooltip, and clicking it copies that reason.
 *
 * **Controlled**, because the text has to change under the pointer after a copy and go back once it leaves, which
 * Radix's own open state knows nothing about. The copy is made from the click itself, which is the user gesture the
 * clipboard asks for.
 */
const FailedPill = ({ reason }: { reason: string }) => {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const [copied, setCopied] = useState(false);

    const copy = () =>
        navigator.clipboard.writeText(reason).then(
            () => setCopied(true),
            // The tooltip keeps showing the reason, so the user can still read it.
            (error: unknown) => report("copying the reason an export failed was refused", error),
        );

    return (
        <Tooltip open={open}>
            <TooltipTrigger
                type="button"
                onPointerEnter={() => setOpen(true)}
                onPointerLeave={() => {
                    setOpen(false);
                    setCopied(false);
                }}
                onFocus={() => setOpen(true)}
                onBlur={() => {
                    setOpen(false);
                    setCopied(false);
                }}
                onClick={() => void copy()}
                data-slot="export-stage"
                className="flex h-5 cursor-pointer items-center gap-1.25 rounded-full bg-red-500/14 px-2 text-red-400 text-xs transition-colors hover:bg-red-500/24"
            >
                <CircleAlert className="size-3" strokeWidth={2} aria-hidden="true" />
                {t("export.queue.failed")}
            </TooltipTrigger>
            <TooltipContent
                side="left"
                sideOffset={8}
                // Red rather than the application's inverted tooltip, as the design draws it; the arrow follows
                // the surface through the descendant selector, which outranks the arrow's own classes.
                className="w-75 bg-red-600 px-2.5 py-2 text-left text-white text-xs/[1.45] text-pretty [&_svg]:bg-red-600 [&_svg]:fill-red-600"
            >
                {copied ? t("export.queue.copied") : reason}
            </TooltipContent>
        </Tooltip>
    );
};

/** What a row's pill reads, by stage. */
const StagePill = ({ row }: { row: Row }) => {
    const { t } = useTranslation();

    if (row.stage === "failed") return <FailedPill reason={row.reason} />;

    return (
        <span
            data-slot="export-stage"
            className={cn("flex h-5 items-center rounded-full px-2 text-xs", STAGE_PILL[row.stage])}
        >
            {t(`export.queue.${row.stage}`)}
        </span>
    );
};

/** The control that shows a written file in the file manager, named as the drawer's menu names the same act. */
const RevealButton = ({ path }: { path: string }) => {
    const { t } = useTranslation();

    // A rejection is either a file manager that would not open or a file since moved; both leave the file where the
    // export wrote it, which is what the notice says.
    const reveal = () =>
        revealExport(path).catch((error: unknown) => {
            report("revealing an exported file failed", error);
            toast.error(t("errors.revealFailed"));
        });

    return (
        <button
            type="button"
            onClick={() => void reveal()}
            aria-label={showInLabel(t)}
            className="flex size-7 flex-none items-center justify-center rounded-md bg-secondary text-muted-foreground transition-colors hover:bg-input hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
        >
            <Folder className="size-4" aria-hidden="true" />
        </button>
    );
};

/**
 * One queued file: its thumbnail, the name it will be written under, its dimensions before and after, its sizes, its
 * types and its stage.
 *
 * **What it shows is read live** - the name from the export settings, the dimensions from the stack and the framing -
 * so a change while idle is seen at once, and the column being disabled while running keeps the names the ones
 * being written. See design.md D1.
 *
 * Only the row being worked on has a bar of its own, and only a written row a way to reveal its file. While `follow`
 * holds, the row being worked on also keeps itself in view.
 */
export const ExportRow = ({ path, follow = false }: { path: string; follow?: boolean }) => {
    const file = useFileByPath(path);
    const row = useExportBatchStore((state) => state.rows.get(path)) ?? QUEUED;
    const current = useExportBatchStore((state) => state.current?.path === path);
    const stack = useFileEnhancements(path);
    const crop = useImageCrop(file?.identity);
    const prefix = useExportSettingsStore((state) => state.prefix);
    const suffix = useExportSettingsStore((state) => state.suffix);
    const format = useExportSettingsStore((state) => state.format);
    const formats = useExportFormats();
    const rowRef = useRef<HTMLDivElement>(null);

    // The row being worked on is kept in view, so a batch longer than the queue's height never runs out of sight.
    useEffect(() => {
        if (current && follow) rowRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }, [current, follow]);

    // Closing a photograph is not reachable behind the dialog; a record gone anyway has nothing to describe. The
    // formats' rules are Rust's and arrive once, a moment after the dialog opens; a row waits for them rather than
    // naming a file by a rule of its own.
    if (!file || !formats) return;

    const name = exportNameFor(file, { prefix, suffix }, format, formats);
    const { width, height } = framedDimensions(file, crop);
    const scale = upscaleFactor(stack);
    const types = `${file.extension.toUpperCase()} → ${extensionFor(file, format, formats).toUpperCase()}`;
    const fraction = row.stage === "enhancing" ? row.fraction : row.stage === "writing" ? 1 : 0;

    return (
        <div
            ref={rowRef}
            data-slot="export-row"
            className={cn(
                "flex flex-none flex-col gap-2 rounded-lg border border-border p-2",
                current && "bg-surface-thumbnail",
            )}
        >
            <div className="flex items-center gap-3">
                <div className="size-12 flex-none overflow-hidden rounded-sm bg-surface-thumbnail">
                    {file.identity && (
                        <img
                            alt={name}
                            src={renditionFor(file, ROW_THUMBNAIL_BOUND, crop)}
                            className="size-full object-cover"
                        />
                    )}
                </div>

                <div className="flex min-w-0 flex-1 flex-col gap-0.75">
                    <span className="truncate text-[13px]">{name}</span>
                    <span className="flex items-center gap-2 text-foreground-dim text-xs">
                        {width !== undefined && height !== undefined && (
                            <>
                                {formatDimensions(width, height)}
                                <ArrowRight className="size-3" strokeWidth={2} aria-hidden="true" />
                                <span className="text-zinc-200">
                                    {formatDimensions(Math.round(width * scale), Math.round(height * scale))}
                                </span>
                                <span className="text-input">·</span>
                            </>
                        )}
                        {types}
                    </span>
                </div>

                <div className="flex w-47.5 flex-none flex-col items-end gap-0.75">
                    <StagePill row={row} />
                    <span className="flex items-center gap-1.5 text-foreground-dim text-xs">
                        {/* A dash where the size could not be read, which needs no translation. */}
                        {file.size !== undefined ? formatBytes(file.size) : "—"}
                        {row.stage === "done" && (
                            <>
                                <ArrowRight className="size-3" strokeWidth={2} aria-hidden="true" />
                                <span className="text-zinc-200">{formatBytes(row.bytes)}</span>
                            </>
                        )}
                    </span>
                </div>

                {row.stage === "done" ? <RevealButton path={row.path} /> : <span className="size-7 flex-none" />}
            </div>

            {current && (
                <Progress value={fraction * 100} aria-label={name} className="h-0.5 rounded-none bg-secondary" />
            )}
        </div>
    );
};
