import type { File } from '../../bindings/gui/types';
import { GetImage, ProcessImage } from '../../bindings/gui/services/imageservice.ts';

export type ImageData = {
    url: string;
    width: number;
    height: number;
};

const imageCache = new Map<string, ImageData>();

export const getImage = async (file: File, size: number) => {
    const cacheKey = `${file.Hash}_${size}`;
    let image = imageCache.get(cacheKey);

    if (!image) {
        const [base64, width, height] = await GetImage(file.Path, size);
        image = await createImageData(base64, width, height);
        imageCache.set(cacheKey, image);
    }

    return image;
};

export const getEnhancedImage = async (file: File, ...operations: string[]) => {
    const opIds = operations.join('_');
    const cacheKey = `${file.Hash}_${opIds}`;
    let image = imageCache.get(cacheKey);

    if (!image) {
        const [base64, width, height] = await ProcessImage(file.Path, ...operations);
        image = await createImageData(base64, width, height);
        imageCache.set(cacheKey, image);
    }

    return image;
};

export const clearCache = () => {
    imageCache.clear();
};

const createImageData = async (base64: string, width: number, height: number): Promise<ImageData> => {
    const response = await fetch(`data:application/octet-stream;base64,${base64}`);
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);

    return { url, width, height };
};
