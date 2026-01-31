import { basename, dirname, extname, join } from 'pathe';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { type ExecutionProvider, ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { ExportImage } from '@/bindings/gui/services/imageservice.ts';

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

export const exportImage = (
    file: File,
    ep: ExecutionProvider,
    operations: Operation[],
    overwrite: boolean,
    format: string,
    prefix: string,
    suffix: string,
    location?: string,
) => {
    const { filePath, ext } = getExportInfo(file, format, prefix, suffix, location);
    const imgFormat = getImageFormat(ext);
    const opIds = operations.map((op) => op.id);

    return ExportImage(file, filePath, ep, overwrite, imgFormat, ...opIds);
};

const getImageFormat = (ext: string) => {
    switch (ext) {
        case 'avif':
            return ImageFormat.FormatAvif;

        case 'bmp':
            return ImageFormat.FormatBmp;

        case 'gif':
            return ImageFormat.FormatGif;

        case 'heic':
        case 'heif':
            return ImageFormat.FormatHeic;

        case 'jpg':
        case 'jpeg':
            return ImageFormat.FormatJpeg;

        case 'png':
            return ImageFormat.FormatPng;

        case 'tif':
        case 'tiff':
            return ImageFormat.FormatTiff;

        case 'webp':
            return ImageFormat.FormatWebp;

        default:
            throw new Error(`Unsupported image format: ${ext}`);
    }
};
