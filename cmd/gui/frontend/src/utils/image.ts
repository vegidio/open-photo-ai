import { CancellablePromise } from '@wailsio/runtime';
import { LRUCache } from 'lru-cache';
import type { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { GetImage, ProcessImage } from '@/bindings/gui/services/imageservice.ts';
import { type CropInfo, type File, InferenceParams } from '@/bindings/gui/types';
import { useCropStore } from '@/stores/crop.ts';
import { useEnhancementStore } from '@/stores/enhancements.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { getEnabledFaces, hasFaceRecovery } from '@/utils/face.ts';

export type ImageData = {
    id: string;
    url: string;
    width: number;
    height: number;
};

const imageCache = new LRUCache<string, ImageData>({ max: 1000 });

// A stable cache-key fragment for a crop; empty string when there's no crop so uncropped keys stay unchanged.
const cropToken = (crop?: CropInfo) =>
    crop
        ? `_c${crop.Rotation}-${crop.FlipH ? 1 : 0}${crop.FlipV ? 1 : 0}-${crop.Left}-${crop.Top}-${crop.Width}-${crop.Height}`
        : '';

// The source dimensions of a file: the crop box (post-rotation) when cropped, otherwise the file's own dimensions.
export const cropDimensions = (file: File, crop?: CropInfo): [number, number] =>
    crop ? [crop.Width, crop.Height] : [file.Dimensions[0], file.Dimensions[1]];

/**
 * Retrieves an image with the specified size, using a cache to avoid redundant processing.
 *
 * @param file - The file object containing the image path and hash.
 * @param size - The desired size for the image.
 * @param crop - Optional flip/rotate/crop to apply; only honored by the backend when size is 0.
 * @returns A promise that resolves to the image data (url, width, height).
 */
export const getImage = async (file: File, size: number, crop?: CropInfo) => {
    const cacheKey = `${file.Hash}_${size}${cropToken(crop)}`;
    let image = imageCache.get(cacheKey);

    if (!image) {
        const [base64, width, height] = await GetImage(file.Path, size, crop ?? EMPTY_CROP);
        image = await createImageData(file.Hash, base64, width, height);
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
 * @param ep - The execution provider to use for image processing.
 * @param operations - The image processing operations to apply.
 * @returns A cancellable promise that resolves to the image data (url, width, height).
 */
export const getEnhancedImage = (file: File, ep: ExecutionProvider, ...operations: string[]) => {
    const opIds = operations.join('_');

    // The user can deselect individual faces; that selection changes the recovery output, so it must be part of the
    // cache key (otherwise a toggle would return a stale enhanced image).
    const isFaceRecovery = hasFaceRecovery(operations);
    const disabled = isFaceRecovery ? useEnhancementStore.getState().disabledFaces.get(file) : undefined;
    const faceToken = disabled?.size ? `_d${[...disabled].sort((a, b) => a - b).join('-')}` : '';

    // The crop is applied to the source before enhancement, so it must be part of the cache key too.
    const crop = useCropStore.getState().crops.get(file);
    const cacheKey = `${file.Hash}_${opIds}${faceToken}${cropToken(crop)}`;

    let image = imageCache.get(cacheKey);
    let p: CancellablePromise<[string, number, number]>;

    return new CancellablePromise<ImageData>(
        async (resolve, reject) => {
            if (!image) {
                try {
                    // Face recovery no longer detects faces internally; detect them up front (cached by hash) and pass
                    // them along so the recovery operations receive them — minus any faces the user has deselected.
                    const faces = await getEnabledFaces(file, ep, operations, disabled);

                    p = ProcessImage(
                        file.Path,
                        ep,
                        new InferenceParams({ Faces: faces, Crop: crop ?? EMPTY_CROP }),
                        ...operations,
                    );
                    const [base64, width, height] = await p;
                    image = await createImageData(file.Hash, base64, width, height);
                    imageCache.set(cacheKey, image);

                    resolve(image);
                } catch (e) {
                    reject(e);
                }
            } else {
                resolve(image);
            }
        },
        () => p?.cancel(),
    );
};

export const clearCache = () => {
    imageCache.clear();
};

const createImageData = async (id: string, base64: string, width: number, height: number): Promise<ImageData> => {
    const response = await fetch(`data:application/octet-stream;base64,${base64}`);
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);

    return { id, url, width, height };
};
