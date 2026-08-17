import { CancelError, CancellablePromise } from '@wailsio/runtime';
import { LRUCache } from 'lru-cache';
import type { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { GetImage, ProcessImage } from '@/bindings/gui/services/imageservice.ts';
import { type CropInfo, type File, InferenceParams } from '@/bindings/gui/types';
import { useCropStore } from '@/stores/crop.ts';
import { useEnhancementStore } from '@/stores/enhancements.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { cropToken } from '@/utils/crop.ts';
import { getEnabledFaces, hasFaceRecovery } from '@/utils/face.ts';

export type ImageData = {
    id: string;
    url: string;
    width: number;
    height: number;
    // Byte size of the decoded blob behind `url`. Carried on the entry purely so the cache below can bound itself by
    // memory; nothing else reads it.
    bytes: number;
};

// ~1.5 GB of decoded image blobs. A full-resolution preview of a 4x upscale is tens of MB, so bounding by entry count
// alone would let the cache hold multiple GB before evicting anything.
const MAX_CACHE_BYTES = 1_500_000_000;

// Two things bound this cache, and both are load-bearing. `maxSize`/`sizeCalculation` bound the actual memory, since
// the entries differ in size by orders of magnitude (a drawer thumbnail vs. a full-resolution upscale) and `max` alone
// says nothing about how much is resident. `dispose` is what releases an evicted entry: the object URL is the only
// remaining reference to the decoded blob, so dropping an entry without revoking it would strand that image for the
// lifetime of the webview. `max` stays as a backstop against unbounded growth in tiny thumbnails.
const imageCache = new LRUCache<string, ImageData>({
    max: 1000,
    maxSize: MAX_CACHE_BYTES,
    // Must never return 0 — lru-cache rejects a zero size — hence the floor.
    sizeCalculation: (value) => Math.max(1, value.bytes),
    dispose: (value) => URL.revokeObjectURL(value.url),
});

// Requests that have been issued but haven't resolved yet, keyed exactly like the cache.
//
// Without this, two concurrent misses on the same key both fetch and both `set`, and the second `set` fires `dispose`
// on the first value — revoking an object URL the first caller has already returned and rendered, blanking the image.
// Callers that arrive during a fetch join it instead of starting a second one.
const inFlight = new Map<string, Promise<ImageData>>();

// The source dimensions of a file: the crop box (post-rotation) when cropped, otherwise the file's own dimensions.
export const cropDimensions = (file: File, crop?: CropInfo): [number, number] =>
    crop ? [crop.Width, crop.Height] : [file.Dimensions[0], file.Dimensions[1]];

/**
 * Publishes a freshly produced image, unless an equivalent entry landed while it was being produced.
 *
 * Writing over an existing entry fires `dispose` on the incumbent, revoking an object URL another caller may already
 * be rendering. So the incumbent wins and the duplicate is released instead — the two are interchangeable anyway,
 * since the key covers everything that determines the pixels.
 */
const commit = (key: string, image: ImageData): ImageData => {
    const existing = imageCache.get(key);

    if (existing) {
        URL.revokeObjectURL(image.url);
        return existing;
    }

    imageCache.set(key, image);
    return image;
};

/**
 * Resolves a cached image, or runs `produce` to create it — at most once per key at a time.
 *
 * Only for producers that can't be cancelled: callers arriving during a fetch join the same promise, so they share its
 * outcome. `getEnhancedImage` deliberately doesn't use this, because one caller cancelling would reject the others.
 */
const cached = (key: string, produce: () => Promise<ImageData>): Promise<ImageData> => {
    const image = imageCache.get(key);
    if (image) return Promise.resolve(image);

    const pending = inFlight.get(key);
    if (pending) return pending;

    const promise = produce()
        .then((result) => commit(key, result))
        .finally(() => inFlight.delete(key));

    inFlight.set(key, promise);
    return promise;
};

/**
 * Retrieves an image with the specified size, using a cache to avoid redundant processing.
 *
 * `size` caps the longest dimension; 0 means the original dimensions. `crop` is only honored by the backend when
 * `size` is 0.
 */
export const getImage = (file: File, size: number, crop?: CropInfo) => {
    const cacheKey = `${file.Hash}_${size}${cropToken(crop)}`;

    return cached(cacheKey, async () => {
        const [base64, width, height] = await GetImage(file.Path, size, crop ?? EMPTY_CROP);
        return createImageData(file.Hash, base64, width, height);
    });
};

/**
 * Retrieves an enhanced image with the specified operations applied, using a cache to avoid redundant processing.
 *
 * The returned promise is cancellable, so a preview the user has navigated away from can be aborted mid-inference.
 */
export const getEnhancedImage = (file: File, ep: ExecutionProvider, ...operations: string[]) => {
    const opIds = operations.join('_');

    // The user can deselect individual faces; that selection changes the recovery output, so it must be part of the
    // cache key (otherwise a toggle would return a stale enhanced image).
    const isFaceRecovery = hasFaceRecovery(operations);
    const disabled = isFaceRecovery ? useEnhancementStore.getState().disabledFaces.get(file.Path) : undefined;
    const faceToken = disabled?.size ? `_d${[...disabled].sort((a, b) => a - b).join('-')}` : '';

    // The crop is applied to the source before enhancement, so it must be part of the cache key too.
    const crop = useCropStore.getState().crops.get(file.Path);
    const cacheKey = `${file.Hash}_${opIds}${faceToken}${cropToken(crop)}`;

    // Tracked separately from `p` because cancellation can arrive before there is anything to cancel: `p` only exists
    // after face detection has resolved. Without the flag, cancelling during detection would reject this promise and
    // then still launch a full inference that nobody is waiting for.
    let cancelled = false;
    let p: CancellablePromise<[string, number, number]> | undefined;

    return new CancellablePromise<ImageData>(
        async (resolve, reject) => {
            const image = imageCache.get(cacheKey);
            if (image) return resolve(image);

            try {
                // Face recovery no longer detects faces internally; detect them up front (cached by hash+crop)
                // and pass them along so the recovery operations receive them — minus any faces the user
                // deselected.
                const faces = await getEnabledFaces(file, ep, operations, disabled, crop);
                if (cancelled) return reject(new CancelError());

                p = ProcessImage(
                    file.Path,
                    ep,
                    new InferenceParams({ Faces: faces, Crop: crop ?? EMPTY_CROP }),
                    ...operations,
                );

                const [base64, width, height] = await p;
                resolve(commit(cacheKey, await createImageData(file.Hash, base64, width, height)));
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

export const clearCache = () => {
    // `clear()` runs `dispose` for every entry, so the blobs are released along with the entries. In-flight requests
    // are dropped too, so one that lands after the clear repopulates nothing.
    imageCache.clear();
    inFlight.clear();
};

const createImageData = async (id: string, base64: string, width: number, height: number): Promise<ImageData> => {
    const response = await fetch(`data:application/octet-stream;base64,${base64}`);
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);

    return { id, url, width, height, bytes: blob.size };
};
