import type { TFunction } from "i18next";
import type { Operation } from "@/ipc/enhance";
import type { ExportFormat, ExportFormats, ExportNaming, FormatChoice, QualityRange } from "@/ipc/export";
import { directoryOf, fileName } from "@/lib/paths";

export type { FormatChoice };

/** Every format a user can pick, in the order the chooser offers them: Preserve, then alphabetical by label. */
export const FORMAT_CHOICES = [
    "preserve",
    "avif",
    "bmp",
    "gif",
    "heic",
    "jpeg",
    "png",
    "tiff",
    "webp",
] as const satisfies readonly FormatChoice[];

export const isFormatChoice = (value: unknown): value is FormatChoice =>
    typeof value === "string" && (FORMAT_CHOICES as readonly string[]).includes(value);

/**
 * Only the naming's own fields of `settings` - the prefix, suffix, folder and format - whatever else it holds, and no
 * `location` where none is chosen. What `exportNames` and `exportImage` send, and what the dialog asks names for.
 */
export const exportNaming = ({
    prefix,
    suffix,
    location,
    format,
}: {
    prefix: string;
    suffix: string;
    location?: string | undefined;
    format: FormatChoice;
}): ExportNaming => ({
    prefix,
    suffix,
    ...(location !== undefined && { location }),
    format,
});

/**
 * The paths reordered so those whose stacks run the same enhancements, in the same order, are exported back to back.
 *
 * The backend keeps models loaded between photographs, bounded by a memory budget, so a batch of one chain loads
 * each model once. A mixed batch whose models do not all fit at once would evict a model a later photograph needs
 * back, which is a whole session rebuilt. Grouping bounds every model to one load per batch however the budget falls.
 *
 * The signature is the families alone: a scale or an intensity is handed to the loaded model on each call, so two
 * stacks differing only there share it. Groups keep the order of their first member and members keep theirs, so the
 * order is predictable rather than a sort's. A path with no stack has the empty signature, and those group together.
 */
export const groupByChain = (paths: readonly string[], stackOf: (path: string) => readonly Operation[] | undefined) => {
    const groups = new Map<string, string[]>();

    for (const path of paths) {
        const signature = (stackOf(path) ?? []).map((operation) => operation.family).join(">");
        const group = groups.get(signature);

        if (group) group.push(path);
        else groups.set(signature, [path]);
    }

    return [...groups.values()].flat();
};

/**
 * The range of the quality `format` is written at, with the format, or `undefined` where that format takes none, as
 * `formats` publishes it. `format` is the one Rust resolved for the file - see `exportNames`.
 */
export const qualityFor = (
    format: ExportFormat,
    formats: ExportFormats,
): { format: ExportFormat; range: QualityRange } | undefined => {
    const range = formats.formats.find((published) => published.format === format)?.quality;

    return range ? { format, range } : undefined;
};

/**
 * The one lossy format every queued file is written in, and its range, or `undefined` where there is none.
 *
 * `written` is the format each queued file is written in, as Rust resolved it. One slider cannot honestly stand for
 * two formats, whose scales are not comparable, so under a mixed Preserve it is hidden and each file is written at
 * its own format's quality. A lossless format, and an empty queue, have nothing to show either.
 */
export const queueQualityFor = (
    written: Iterable<ExportFormat>,
    formats: ExportFormats,
): { format: ExportFormat; range: QualityRange } | undefined => {
    let common: { format: ExportFormat; range: QualityRange } | undefined;

    for (const format of written) {
        const quality = qualityFor(format, formats);

        if (quality === undefined || (common !== undefined && quality.format !== common.format)) return undefined;
        common = quality;
    }

    return common;
};

/**
 * Why one queued file could not be exported.
 *
 * - `export`: the `export` command rejected, with its `ExportError` - typed `unknown`, as an `invoke` rejection is.
 * - `unreadable`: the record has no identity, so nothing can address its pixels and nothing was asked for.
 * - `analysis`: the Autopilot analysis the export asked for failed, with whatever it failed with.
 */
export type ExportFailure =
    | { cause: "export"; error: unknown }
    | { cause: "unreadable" }
    | { cause: "analysis"; error?: unknown };

/** The `kind`, `message` and `path` of a rejection shaped as the backend's errors are, as far as it has them. */
const shapeOf = (error: unknown): { kind?: string; message?: string; path?: string } => {
    if (error === null || typeof error !== "object") return typeof error === "string" ? { message: error } : {};

    const { kind, message, path } = error as { kind?: unknown; message?: unknown; path?: unknown };

    return {
        ...(typeof kind === "string" && { kind }),
        ...(typeof message === "string" && { message }),
        ...(typeof path === "string" && { path }),
    };
};

/** A translated sentence, followed by the untranslated detail where there is one. */
const followedBy = (sentence: string, detail: string | undefined) => (detail ? `${sentence} ${detail}` : sentence);

/**
 * What a Failed row says, composed once, when the row fails: the tooltip draws it and a click copies it.
 *
 * The sentence is translated and the detail after it is not. The detail is the operating system's or the library's
 * own, a diagnostic to be copied into a bug report, and a translated one is one nobody reading the report can search
 * for. A write names the file and the folder of the error's own `path`: the destination Rust asked for.
 *
 * A failed model download arrives as `enhance`, whose message names the download; it has no reason of its own.
 */
export const describeExportError = (t: TFunction, failure: ExportFailure): string => {
    if (failure.cause === "unreadable") return t("export.queue.unreadable");

    const { kind, message, path } = shapeOf(failure.error);

    if (failure.cause === "analysis") return followedBy(t("export.queue.analysisFailed"), message);

    if (kind === "write" && path !== undefined) {
        const written = t("export.queue.writeFailed", { name: fileName(path), folder: directoryOf(path) });

        return followedBy(written, message);
    }

    // `enhance` and `unreadableSource` carry the library's sentence. `notReady`, `unknownSource` and
    // `unknownOperation` are this window's own mistakes rather than the user's, and carry their kind for the report.
    return followedBy(t("export.queue.enhanceFailed"), message ?? kind);
};
