import type { TFunction } from "i18next";
import type { Operation } from "@/ipc/enhance";
import type { ExportFormat, ExportFormats, QualityRange } from "@/ipc/export";
import type { ImageRecord } from "@/ipc/images";
import { directoryOf, fileName } from "@/lib/paths";

/** What the user picks as the format: the source's own where it can be written, or one format for every file. */
export type FormatChoice = "preserve" | ExportFormat;

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

/** What decides the name and folder of each file written, as the export settings hold it. */
export type NamingChoices = {
    prefix: string;
    suffix: string;
    /** The folder to write into, or absent for the directory each source is in. */
    location?: string;
};

/** What `formats` publishes about `format`, or `undefined` where it publishes nothing. */
const capabilityOf = (formats: ExportFormats, format: ExportFormat) =>
    formats.formats.find((published) => published.format === format);

/** The format a Preserve export writes `file` back as, keeping its own extension, or `undefined` where none does. */
const preservedAs = (file: ImageRecord, formats: ExportFormats) =>
    formats.formats.find((published) => published.preserves.includes(file.extension));

/**
 * The format `file` is written as under `choice`.
 *
 * Preserve keeps the source's own format where one is published as writing it back, and writes the published
 * fallback - TIFF, for a camera RAW file - otherwise. **The rules are Rust's**, read off `formats`: which extensions a
 * format writes back is `export_formats`' answer, so this names no format of its own.
 */
export const formatFor = (file: ImageRecord, choice: FormatChoice, formats: ExportFormats): ExportFormat => {
    if (choice !== "preserve") return choice;

    return preservedAs(file, formats)?.format ?? formats.fallback;
};

/**
 * The extension `file` is written with under `choice`, without its dot.
 *
 * Preserve keeps the source's own extension where its format is written back, `heif` staying `heif`, and a source
 * written as the fallback gets the fallback's extension. A chosen format writes its published extension: `jpg` for
 * JPEG, the format's own name for the rest.
 */
export const extensionFor = (file: ImageRecord, choice: FormatChoice, formats: ExportFormats): string => {
    if (choice === "preserve" && preservedAs(file, formats)) return file.extension;

    const format = formatFor(file, choice, formats);

    return capabilityOf(formats, format)?.extension ?? format;
};

/** The source's name without its extension. */
const stemOf = (file: ImageRecord) => {
    const name = fileName(file.path);

    // The record's extension is lowercase and the name need not be (`IMG_0001.JPG`), so it is cut by length.
    return file.extension === "" ? name : name.slice(0, name.length - file.extension.length - 1);
};

/** `name` inside `folder`, joined with the separator the folder is already spelled with. */
const join = (folder: string, name: string) => {
    if (folder === "") return name;
    if (folder.endsWith("/") || folder.endsWith("\\")) return `${folder}${name}`;

    // A folder spelled with backslashes and no slash is a Windows one; everything else joins with a slash, which
    // Windows accepts as well.
    const separator = folder.includes("\\") && !folder.includes("/") ? "\\" : "/";

    return `${folder}${separator}${name}`;
};

/** The name of the file written for `file`: the prefix, the source's name, the suffix and the extension. */
export const exportNameFor = (
    file: ImageRecord,
    choices: NamingChoices,
    choice: FormatChoice,
    formats: ExportFormats,
) => `${choices.prefix}${stemOf(file)}${choices.suffix}.${extensionFor(file, choice, formats)}`;

/**
 * The path the export of `file` asks to write: {@link exportNameFor} inside the chosen folder, or inside the source's
 * own directory where none is chosen.
 *
 * What is asked for, not necessarily what is written: without overwriting, a taken name is written numbered, and the
 * export answers that name.
 */
export const destinationFor = (
    file: ImageRecord,
    choices: NamingChoices,
    choice: FormatChoice,
    formats: ExportFormats,
) => join(choices.location ?? directoryOf(file.path), exportNameFor(file, choices, choice, formats));

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
 * The format whose quality `file` is written at under `choice`, and the range that quality takes - or `undefined`
 * where that format takes none, as `formats` publishes it.
 */
export const qualityFor = (
    file: ImageRecord,
    choice: FormatChoice,
    formats: ExportFormats,
): { format: ExportFormat; range: QualityRange } | undefined => {
    const format = formatFor(file, choice, formats);
    const range = capabilityOf(formats, format)?.quality;

    return range ? { format, range } : undefined;
};

/**
 * The one lossy format every queued file is written in, and its range, or `undefined` where there is none.
 *
 * One slider cannot honestly stand for two formats, whose scales are not comparable, so under a mixed Preserve it is
 * hidden and each file is written at its own format's quality. A lossless format, and an empty queue, have nothing to
 * show either.
 */
export const queueQualityFor = (
    files: Iterable<ImageRecord>,
    choice: FormatChoice,
    formats: ExportFormats,
): { format: ExportFormat; range: QualityRange } | undefined => {
    let common: { format: ExportFormat; range: QualityRange } | undefined;

    for (const file of files) {
        const quality = qualityFor(file, choice, formats);

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

/** The `kind` and `message` of a rejection shaped as the backend's errors are, as far as it has them. */
const shapeOf = (error: unknown): { kind?: string; message?: string } => {
    if (error === null || typeof error !== "object") return typeof error === "string" ? { message: error } : {};

    const { kind, message } = error as { kind?: unknown; message?: unknown };

    return {
        ...(typeof kind === "string" && { kind }),
        ...(typeof message === "string" && { message }),
    };
};

/** A translated sentence, followed by the untranslated detail where there is one. */
const followedBy = (sentence: string, detail: string | undefined) => (detail ? `${sentence} ${detail}` : sentence);

/**
 * What a Failed row says, composed once, when the row fails: the tooltip draws it and a click copies it.
 *
 * The sentence is translated and the detail after it is not. The detail is the operating system's or the library's
 * own, a diagnostic to be copied into a bug report, and a translated one is one nobody reading the report can search
 * for. A write names the file and the folder of `destination`, the path asked for.
 *
 * A failed model download arrives as `enhance`, whose message names the download; it has no reason of its own.
 */
export const describeExportError = (t: TFunction, failure: ExportFailure, destination: string): string => {
    if (failure.cause === "unreadable") return t("export.queue.unreadable");

    const { kind, message } = shapeOf(failure.error);

    if (failure.cause === "analysis") return followedBy(t("export.queue.analysisFailed"), message);

    if (kind === "write") {
        const written = t("export.queue.writeFailed", {
            name: fileName(destination),
            folder: directoryOf(destination),
        });

        return followedBy(written, message);
    }

    // `enhance` and `unreadableSource` carry the library's sentence. `notReady`, `unknownSource` and
    // `unknownOperation` are this window's own mistakes rather than the user's, and carry their kind for the report.
    return followedBy(t("export.queue.enhanceFailed"), message ?? kind);
};
