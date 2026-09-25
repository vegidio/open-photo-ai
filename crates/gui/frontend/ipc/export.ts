import { listen } from "@tauri-apps/api/event";
import type { CropInfo } from "./crop";
import { type EnhanceError, mintRun, type Operation, type Processor, type RunProgress } from "./enhance";
import { call } from "./invoke";
import { once } from "./once";

// Written a second time in `crates/gui/src/export/`, as `#[tauri::command]` function names: the first two in
// `mod.rs`, then `directory.rs` and `written.rs`. A rename on one side alone is not a type error but an `invoke` that
// rejects at runtime: `export.test.ts` pins this half, and `generate_handler![]` the Rust one by refusing to compile
// against a function that does not exist.
const EXPORT_COMMAND = "export";
const CANCEL_EXPORT_COMMAND = "cancel_export";
const PICK_DIRECTORY_COMMAND = "pick_directory";
const REVEAL_EXPORT_COMMAND = "reveal_export";
const EXPORT_FORMATS_COMMAND = "export_formats";

/**
 * The event an export's reports arrive under, written a second time in `crates/gui/src/export/progress.rs` as
 * `EXPORT_PROGRESS_EVENT`.
 *
 * **Not `enhance:progress`.** That one drives the canvas chip, and an export must never move it, so an export's
 * reports go somewhere no canvas listener is subscribed.
 */
const EXPORT_PROGRESS_EVENT = "export:progress";

/**
 * The formats a photograph can be exported as, as the window names them. What each can do - whether it takes a
 * quality, what it is written as - is {@link ExportFormats}, published by Rust. `heic` is written by the library as
 * HEIF.
 */
export type ExportFormat = "bmp" | "gif" | "jpeg" | "png" | "tiff" | "avif" | "heic" | "webp";

/** What a lossy format's quality may be, and what it is written at before a user moves a slider. */
export type QualityRange = { min: number; max: number; default: number };

/** What one format can do, as Rust's `FormatCapability` publishes it. */
export type FormatCapability = {
    format: ExportFormat;
    /** The extension a file written in it is given when it is chosen, without its dot: `jpg` for JPEG. */
    extension: string;
    /**
     * The source extensions a Preserve export writes back **as this format**, keeping the source's own extension:
     * `jpeg` and `jpg` for JPEG, `heic` and `heif` for HEIC.
     */
    preserves: string[];
    /**
     * The quality it takes, or `null` for a lossless format, whose encoder ignores one. Per format rather than one
     * shared number: the scales are not comparable across encoders.
     */
    quality: QualityRange | null;
};

/**
 * Every format an export can be written in and what each can do, and what a Preserve export writes a source none of
 * them writes back as itself - a camera RAW file - as instead. The one table `lib/export.ts` reads its format rules
 * from, published by `export_formats` in `crates/gui/src/export/mod.rs`.
 */
export type ExportFormats = { formats: FormatCapability[]; fallback: ExportFormat };

/** Where an export is written, and how. */
export type ExportRequest = {
    /** The file to write. Its name does not decide the format; {@link ExportRequest.format} does. */
    destination: string;
    /** What to write. */
    format: ExportFormat;
    /**
     * The quality, for a format {@link ExportFormats} publishes as taking one - absent for the rest, and for a lossy
     * format meant to be written at its published default. A value outside the range is brought inside it rather than
     * refused.
     */
    quality?: number;
    /**
     * Whether a file already at the destination may be replaced, the photograph's own file included. Without it, a
     * taken destination is written as `name_1.ext`, `name_2.ext` and so on.
     */
    overwrite: boolean;
};

/**
 * How an export ended.
 *
 * A stop is **not** a failure, as with {@link enhance}: both resolve, and a caller branches on `outcome`.
 */
export type Exported =
    | {
          outcome: "exported";
          /** The path actually written: the destination, or the numbered name beside it. What a reveal needs. */
          path: string;
          /** How many bytes were written. */
          bytes: number;
      }
    /** The export was stopped before its file was written. Nothing is left at any name it tried. */
    | { outcome: "stopped" };

/**
 * Why the `export` command rejected.
 *
 * Every refusal a canvas run has, with the same shapes, so one formatter describes both. And one it does not:
 * `write`, naming the destination asked for and carrying the operating system's reason, untranslated.
 */
export type ExportError = EnhanceError | { kind: "write"; path: string; message: string };

/**
 * One report about an export in flight.
 *
 * While the chain runs, the report is an enhancement run's own, under the export's name, so everything
 * {@link RunProgress} says holds: a cached operation has no `stage`, a fetch has its own fraction. Once the chain is
 * done, one `writing` report follows, with no fraction because the encoder has none to give. An export with no
 * enhancements reports only that.
 */
export type ExportProgress = (RunProgress & { phase: "enhancing" }) | { run: string; phase: "writing" };

/**
 * Enhance an open image at its own depth and write the result to a file.
 *
 * Answers **the export's name straight away**, beside the promise of its outcome, for the reason {@link enhance}
 * gives: a stop and its export cross the boundary independently, and a cleanup has to be able to name what it is
 * stopping before the promise settles.
 *
 * **An export does not disturb the canvas.** It does not stop the run the window is drawing, and a canvas run does
 * not stop it.
 *
 * `operations` may be empty, which writes the framed photograph as it is. `crop` is how the user has framed it, or
 * absent to export the whole photograph.
 *
 * Rejects with the serialized {@link ExportError}, typed as `unknown` because that is what an `invoke` rejection is.
 */
export const exportImage = (
    source: string,
    operations: Operation[],
    processor: Processor,
    request: ExportRequest,
    crop?: CropInfo,
) => {
    // Before the invoke, for the reason `enhance` gives beside its own. From the one counter every run name comes
    // from, so an export's name can never be a canvas run's or an analysis's.
    const run = mintRun();
    const { destination, format, quality, overwrite } = request;

    // `crop` is sent as `undefined` rather than omitted, as `enhance` sends it.
    return {
        run,
        done: call<Exported>(EXPORT_COMMAND, {
            run,
            source,
            operations,
            processor,
            crop,
            destination,
            format,
            quality,
            overwrite,
        }),
    };
};

// Kept because the Rust side answers from a table fixed at compile time, as the catalogue is.
const formats = once(() => call<ExportFormats>(EXPORT_FORMATS_COMMAND));

/**
 * Every format an export can be written in, and what each can do. **Fetched once and kept**, as {@link once} keeps
 * it: the answer cannot change while the application runs.
 */
export const exportFormats = () => formats.get();

/** Forgets the fetched formats. For tests alone - see {@link once}. */
export const forgetExportFormats = () => formats.forget();

/**
 * Stop an export this window asked for, by the name {@link exportImage} answered.
 *
 * The export then resolves as stopped and writes nothing. A stop that reaches the backend before its own export
 * still stops it. One that arrives once the file is being written lets the write finish, and the export resolves as
 * exported. Stopping one export touches no other, no analysis and no canvas run.
 *
 * Resolves whatever happened; there is no outcome to act on.
 */
export const cancelExport = (run: string) => call<void>(CANCEL_EXPORT_COMMAND, { run });

/**
 * Subscribe to every export's progress reports.
 *
 * One subscription for the application: each report names its export, so a listener matches
 * {@link ExportProgress.run} against the rows it is drawing.
 *
 * Resolves with the function that removes the listener, as `listen` does.
 */
export const onExportProgress = (handler: (report: ExportProgress) => void) =>
    listen<ExportProgress>(EXPORT_PROGRESS_EVENT, (event) => handler(event.payload));

/**
 * Ask the user for a folder to export into, through the platform's own folder picker.
 *
 * `title` arrives already translated, for the reason `openImages` gives. Resolves with the folder chosen, or
 * `undefined` where the picker was dismissed, which is **not a failure**: the caller keeps its previous choice.
 */
export const pickDirectory = async (title: string) =>
    // Rust's `None` crosses as `null`; this module answers `undefined` for "no value".
    (await call<string | null>(PICK_DIRECTORY_COMMAND, { title })) ?? undefined;

/**
 * Show a file an export wrote in the platform's own file manager, with the file selected.
 *
 * Takes the path {@link exportImage} answered, the numbered name where one was written, and **the Rust side refuses
 * any path no export of this session wrote**, as `revealImage` refuses one it never opened.
 *
 * The rejection is propagated, for the reason `revealImage` gives.
 */
export const revealExport = (path: string) => call<void>(REVEAL_EXPORT_COMMAND, { path });
