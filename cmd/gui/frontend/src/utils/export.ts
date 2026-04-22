import { basename, dirname, extname, join } from 'pathe';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { type ExecutionProvider, ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { ExportImage } from '@/bindings/gui/services/imageservice.ts';

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

export const getExportEligible = (selectedFiles: File[], enhancements: Map<File, Operation[]>, autopilot: boolean) => {
    const allEnhancements = new Map<File, Operation[]>();

    for (const file of selectedFiles) {
        const operations = enhancements.get(file);
        if (operations && operations.length > 0) allEnhancements.set(file, operations);
        if (!operations && autopilot) allEnhancements.set(file, []);
    }

    return allEnhancements;
};

export const getExportInfo = (file: File, format: string, prefix: string, suffix: string, location?: string) => {
    const basePath = location ?? dirname(file.Path);
    const baseName = basename(file.Path, extname(file.Path));

    const ext = format === 'preserve' ? file.Extension : format;
    const fileName = `${prefix}${baseName}${suffix}.${ext}`;
    const filePath = join(basePath, fileName);

    return { fileName, filePath, ext };
};

export const exportImage = (opts: ExportOptions) => {
    const { file, ep, operations, overwrite, format, prefix, suffix, location } = opts;
    const { filePath, ext } = getExportInfo(file, format, prefix, suffix, location);
    const imgFormat = getImageFormat(ext);
    const opIds = operations.map((op) => op.id);

    return ExportImage(file, filePath, ep, overwrite, imgFormat, ...opIds);
};

const getImageFormat = (ext: string): ImageFormat => {
    const format = IMAGE_FORMAT_BY_EXT[ext];
    if (format === undefined) {
        throw new Error(`Unsupported image format: ${ext}`);
    }
    return format;
};
