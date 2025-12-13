import path from 'path-browserify';
import type { Operation } from '@/operations';
import type { File } from '../../bindings/gui/types';
import { ImageFormat } from '../../bindings/github.com/vegidio/open-photo-ai/types';
import { ExportImage } from '../../bindings/gui/services/imageservice.ts';
import { useExportStore } from '@/stores';

export const exportImage = (file: File, operations: Operation[]) => {
    const format = useExportStore.getState().format;
    const prefix = useExportStore.getState().prefix;
    const suffix = useExportStore.getState().suffix;
    const location = useExportStore.getState().location;

    const basePath = location ?? path.dirname(file.Path);
    const baseName = path.basename(file.Path, path.extname(file.Path));

    const ext = format === 'preserve' ? file.Extension : format;
    const fileName = `${prefix}${baseName}${suffix}.${ext}`;
    const filePath = path.join(basePath, fileName);
    const imgFormat = getImageFormat(ext);
    const opIds = operations.map((op) => op.id);

    return ExportImage(file, filePath, imgFormat, ...opIds);
};

const getImageFormat = (ext: string) => {
    switch (ext) {
        case 'jpg':
        case 'jpeg':
            return ImageFormat.FormatJpeg;

        case 'tif':
        case 'tiff':
            return ImageFormat.FormatTiff;

        default:
            return ImageFormat.FormatPng;
    }
};
