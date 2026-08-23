import { CancelError, CancellablePromise } from '@wailsio/runtime';
import { basename, dirname, extname, join } from 'pathe';
import type { Operation } from '@/operations';
import { type ExecutionProvider, ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { ExportImage } from '@/bindings/gui/services/imageservice.ts';
import { type File, InferenceParams } from '@/bindings/gui/types';
import { useCropStore } from '@/stores/crop.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { getEnabledFaces } from '@/utils/face.ts';
import { LOSSLESS_QUALITY, type QualityChoices, type QualityFormat, QUALITY_FORMATS } from '@/utils/quality.ts';

export type ExportOptions = {
    file: File;
    ep: ExecutionProvider;
    operations: Operation[];
    overwrite: boolean;
    format: string;
    prefix: string;
    suffix: string;
    location?: string;

    // The whole record, not one number: under "preserve" each file resolves to its own format, so the quality is
    // picked per file rather than once for the batch.
    quality: QualityChoices;
};

const IMAGE_FORMAT_BY_EXT: Record<string, ImageFormat> = {
    avif: ImageFormat.FormatAvif,
    bmp: ImageFormat.FormatBmp,
    gif: ImageFormat.FormatGif,
    heic: ImageFormat.FormatHeic,
    heif: ImageFormat.FormatHeic,
    jpg: ImageFormat.FormatJpeg,
    jpeg: ImageFormat.FormatJpeg,
    png: ImageFormat.FormatPng,
    tif: ImageFormat.FormatTiff,
    tiff: ImageFormat.FormatTiff,
    webp: ImageFormat.FormatWebp,
};

/**
 * The format an export will encode `file` as, or undefined when that format has no quality setting.
 *
 * Resolving to the ImageFormat rather than the extension is what makes the aliases collapse: `jpg` and `jpeg` are one
 * format with one stored quality, as are `heic` and `heif`.
 */
export const getQualityFormat = (file: File, format: string): QualityFormat | undefined => {
    const imageFormat = IMAGE_FORMAT_BY_EXT[resolveExt(file, format)];
    return QUALITY_FORMATS.includes(imageFormat as QualityFormat) ? (imageFormat as QualityFormat) : undefined;
};

/**
 * The single format a Quality slider would apply to for the whole queue, or undefined when there isn't one.
 *
 * Under "preserve" the queue can hold files of different formats, and one slider cannot honestly stand for all of
 * them - so it is hidden, and each file falls back to its own stored quality at export time. A queue that resolves to
 * a lossless format, and an empty queue, have nothing to show either.
 */
export const resolveQualityFormat = (files: Iterable<File>, format: string): QualityFormat | undefined => {
    let common: QualityFormat | undefined;

    for (const file of files) {
        const qualityFormat = getQualityFormat(file, format);
        if (qualityFormat === undefined) return undefined;
        if (common !== undefined && qualityFormat !== common) return undefined;
        common = qualityFormat;
    }

    return common;
};

/**
 * Picks the files an export should actually process: those with enhancements, plus (under Autopilot) those with none
 * yet, whose operations are suggested later.
 *
 * `enhancements` is the store's map, keyed by path. The result is keyed by the `File` objects instead, because the
 * export needs the whole file (path, hash, extension) — that map is built and consumed within a single pass, so
 * object identity is not a concern there the way it is for the long-lived stores.
 */
export const getExportEligible = (
    selectedFiles: File[],
    enhancements: Map<string, Operation[]>,
    autopilot: boolean,
) => {
    const allEnhancements = new Map<File, Operation[]>();

    for (const file of selectedFiles) {
        const operations = enhancements.get(file.Path);
        if (operations && operations.length > 0) allEnhancements.set(file, operations);
        if (!operations && autopilot) allEnhancements.set(file, []);
    }

    return groupByChain(allEnhancements);
};

/**
 * Reorders the queue so files needing the same models are exported back to back.
 *
 * The backend keeps models loaded between images, bounded by a memory budget. A batch where every file wants the same
 * enhancements therefore loads each model once, however long the batch is. A *mixed* batch is the problem: if the set
 * of models it needs doesn't all fit at once, an image part-way through can evict a model that a later image needs
 * back, and each of those rebuilds is a full ONNX session construction — the exact cost keeping models resident exists
 * to avoid. Grouping bounds each model to one load per batch no matter how the budget falls.
 *
 * This reorders when files are processed, not what they produce. Progress events are keyed by file hash, so the UI is
 * unaffected; only the order rows complete in changes.
 *
 * Autopilot files carry no operations yet — their models are chosen per file once the suggestions come back — so they
 * all share the empty signature and stay in their original relative order.
 */
const groupByChain = (eligible: Map<File, Operation[]>) => {
    // Only the operation ids, not their options: options are per-run parameters (intensity, scale) that the model is
    // handed on each call, so two files differing only there still use the very same loaded model.
    const chainOf = (operations: Operation[]) => operations.map((operation) => operation.id).join('>');

    // Group rather than sort: files with the same chain keep their original order, and so do the groups themselves,
    // which keeps the export order predictable instead of reshuffling on an unrelated change.
    const groups = new Map<string, [File, Operation[]][]>();

    for (const entry of eligible) {
        const chain = chainOf(entry[1]);
        const group = groups.get(chain);

        if (group) group.push(entry);
        else groups.set(chain, [entry]);
    }

    return new Map<File, Operation[]>(Array.from(groups.values()).flat());
};

/**
 * The extension an export will write `file` with.
 *
 * "Preserve" keeps the source extension, but RAW (and any other non-writable) inputs can't be encoded back — fallback
 * to TIFF so the export still produces a valid file.
 *
 * Extracted so the export dialog can predict the outcome with the very same rule the exporter follows; two copies of
 * this would drift, and the slider would then offer a setting the encoder never applies.
 */
export const resolveExt = (file: File, format: string) => {
    const ext = format === 'preserve' ? file.Extension : format;
    return ext in IMAGE_FORMAT_BY_EXT ? ext : 'tiff';
};

export const getExportInfo = (file: File, format: string, prefix: string, suffix: string, location?: string) => {
    const basePath = location ?? dirname(file.Path);
    const baseName = basename(file.Path, extname(file.Path));
    const ext = resolveExt(file, format);

    const fileName = `${prefix}${baseName}${suffix}.${ext}`;
    const filePath = join(basePath, fileName);

    return { fileName, filePath, ext };
};

export const exportImage = (opts: ExportOptions) => {
    const { file, ep, operations, overwrite, format, prefix, suffix, location, quality } = opts;
    const { filePath, ext } = getExportInfo(file, format, prefix, suffix, location);
    const imgFormat = getImageFormat(ext);
    const opIds = operations.map((op) => op.id);

    // Per file, not per batch — see ExportOptions.quality.
    const qualityFormat = getQualityFormat(file, format);
    const imgQuality = qualityFormat === undefined ? LOSSLESS_QUALITY : quality[qualityFormat];

    // Tracked separately from `p` because cancellation can arrive before there is anything to cancel: `p` only exists
    // once face detection has resolved. Without the flag, cancelling during detection would reject this promise and
    // then still launch a full export that nobody is waiting for — per row, for the rest of a cancelled batch.
    let cancelled = false;
    let p: CancellablePromise<void> | undefined;

    return new CancellablePromise<void>(
        async (resolve, reject) => {
            try {
                // Face recovery no longer detects faces internally; detect them up front (cached by hash+crop, so
                // the boxes match the cropped source) and pass them along — minus any faces the user deselected.
                const crop = useCropStore.getState().crops.get(file.Path);
                const faces = await getEnabledFaces(file, ep, opIds, undefined, crop);
                if (cancelled) return reject(new CancelError());

                p = ExportImage(
                    file,
                    filePath,
                    ep,
                    overwrite,
                    imgFormat,
                    imgQuality,
                    new InferenceParams({ Faces: faces, Crop: crop ?? EMPTY_CROP }),
                    ...opIds,
                );
                await p;

                resolve();
            } catch (e) {
                reject(e);
            }
        },
        () => {
            cancelled = true;
            p?.cancel();
        },
    );
};

const getImageFormat = (ext: string): ImageFormat => {
    const format = IMAGE_FORMAT_BY_EXT[ext];
    if (format === undefined) {
        throw new Error(`Unsupported image format: ${ext}`);
    }
    return format;
};
