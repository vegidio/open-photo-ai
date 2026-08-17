import { CancellablePromise } from '@wailsio/runtime';
import { basename, dirname, extname, join } from 'pathe';
import type { Operation } from '@/operations';
import { type ExecutionProvider, ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { ExportImage } from '@/bindings/gui/services/imageservice.ts';
import { type File, InferenceParams } from '@/bindings/gui/types';
import { useCropStore } from '@/stores/crop.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { getEnabledFaces } from '@/utils/face.ts';

export type ExportOptions = {
    file: File;
    ep: ExecutionProvider;
    operations: Operation[];
    overwrite: boolean;
    format: string;
    prefix: string;
    suffix: string;
    location?: string;
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

export const getExportInfo = (file: File, format: string, prefix: string, suffix: string, location?: string) => {
    const basePath = location ?? dirname(file.Path);
    const baseName = basename(file.Path, extname(file.Path));

    // "Preserve" keeps the source extension, but RAW (and any other non-writable) inputs can't be encoded back —
    // fallback to TIFF so the export still produces a valid file.
    let ext = format === 'preserve' ? file.Extension : format;
    if (!(ext in IMAGE_FORMAT_BY_EXT)) ext = 'tiff';

    const fileName = `${prefix}${baseName}${suffix}.${ext}`;
    const filePath = join(basePath, fileName);

    return { fileName, filePath, ext };
};

export const exportImage = (opts: ExportOptions) => {
    const { file, ep, operations, overwrite, format, prefix, suffix, location } = opts;
    const { filePath, ext } = getExportInfo(file, format, prefix, suffix, location);
    const imgFormat = getImageFormat(ext);
    const opIds = operations.map((op) => op.id);

    let p: CancellablePromise<void>;

    return new CancellablePromise<void>(
        async (resolve, reject) => {
            try {
                // Face recovery no longer detects faces internally; detect them up front (cached by hash+crop, so
                // the boxes match the cropped source) and pass them along — minus any faces the user deselected.
                const crop = useCropStore.getState().crops.get(file.Path);
                const faces = await getEnabledFaces(file, ep, opIds, undefined, crop);

                p = ExportImage(
                    file,
                    filePath,
                    ep,
                    overwrite,
                    imgFormat,
                    new InferenceParams({ Faces: faces, Crop: crop ?? EMPTY_CROP }),
                    ...opIds,
                );
                await p;

                resolve();
            } catch (e) {
                reject(e);
            }
        },
        () => p?.cancel(),
    );
};

const getImageFormat = (ext: string): ImageFormat => {
    const format = IMAGE_FORMAT_BY_EXT[ext];
    if (format === undefined) {
        throw new Error(`Unsupported image format: ${ext}`);
    }
    return format;
};
