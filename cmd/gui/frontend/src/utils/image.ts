import { CancellablePromise } from '@wailsio/runtime';
import type { File } from '../../bindings/gui/types';
import { GetImage, ProcessImage } from '../../bindings/gui/services/imageservice.ts';

export type ImageData = {
    url: string;
    width: number;
    height: number;
};

const imageCache = new Map<string, ImageData>();

/**
 * Retrieves an image with the specified size, using a cache to avoid redundant processing.
 *
 * @param file - The file object containing the image path and hash.
 * @param size - The desired size for the image.
 * @returns A promise that resolves to the image data (url, width, height).
 */
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

/**
 * Retrieves an enhanced image with the specified operations applied, using a cache to avoid redundant processing.
 *
 * Returns a cancellable promise that can be aborted if the operation is no longer needed.
 *
 * @param file - The file object containing the image path and hash.
 * @param operations - The image processing operations to apply.
 * @returns A cancellable promise that resolves to the image data (url, width, height).
 */
export const getEnhancedImage = (file: File, ...operations: string[]) => {
    const opIds = operations.join('_');
    const cacheKey = `${file.Hash}_${opIds}`;

    let image = imageCache.get(cacheKey);
    let p: CancellablePromise<[string, number, number]>;

    return new CancellablePromise<ImageData>(
        async (resolve, reject) => {
            if (!image) {
                p = ProcessImage(file.Path, ...operations);

                try {
                    const [base64, width, height] = await p;
                    image = await createImageData(base64, width, height);
                    imageCache.set(cacheKey, image);

                    resolve(image);
                } catch (e) {
                    reject(e);
                }
            } else {
                resolve(image);
            }
        },
        () => p.cancel(),
    );
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
