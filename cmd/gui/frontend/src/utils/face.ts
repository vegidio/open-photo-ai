import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import type { CropInfo, File } from '@/bindings/gui/types';
import { DetectFaces } from '@/bindings/gui/services/faceservice.ts';
import { useEnhancementStore } from '@/stores/enhancements.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { cropToken } from '@/utils/crop.ts';

const facesCache = new Map<string, Face[]>();

/**
 * Detects the faces in an image, caching the result by file hash (plus a crop token) to avoid redundant detection.
 *
 * Detection runs on the cropped image, so the returned bounding boxes live in the cropped image's coordinate space —
 * matching the cropped source that face recovery and the preview operate on. The resulting faces are passed back to
 * the inference calls (ProcessImage/ExportImage). Faces are deterministic for a given image+crop, so caching by hash
 * plus the crop is always safe.
 *
 * @param file - The file object containing the image path and hash.
 * @param ep - The execution provider to use for detection.
 * @param crop - The flip/rotate/crop to detect against; omit for the uncropped image.
 * @returns A promise that resolves to the detected faces (empty when none are found).
 */
export const detectFaces = async (file: File, ep: ExecutionProvider, crop?: CropInfo): Promise<Face[]> => {
    const key = `${file.Hash}${cropToken(crop)}`;
    let faces = facesCache.get(key);

    if (!faces) {
        faces = await DetectFaces(file.Path, ep, crop ?? EMPTY_CROP);
        facesCache.set(key, faces);
    }

    return faces;
};

/**
 * Reports whether any of the given operation IDs is a face-recovery operation (and therefore needs detected faces).
 *
 * @param opIds - The operation IDs to inspect.
 */
export const hasFaceRecovery = (opIds: string[]): boolean => opIds.some((id) => id.startsWith('fr_'));

/**
 * Resolves the faces an inference run should receive: an empty array when no face-recovery operation is present,
 * otherwise the detected faces (cached by hash) minus any the user has deselected.
 *
 * @param file - The file object containing the image path and hash.
 * @param ep - The execution provider to use for detection.
 * @param opIds - The operation IDs of the run (used to decide whether faces are needed at all).
 * @param disabled - An already-read disabled-face selection to reuse (e.g. when the caller also needs it for a cache
 *   key); when omitted, it is read from the enhancement store.
 * @param crop - The flip/rotate/crop to detect against; omit for the uncropped image.
 * @returns A promise that resolves to the enabled faces to pass to the inference call.
 */
export const getEnabledFaces = async (
    file: File,
    ep: ExecutionProvider,
    opIds: string[],
    disabled?: Set<number>,
    crop?: CropInfo,
): Promise<Face[]> => {
    if (!hasFaceRecovery(opIds)) return [];

    const faces = await detectFaces(file, ep, crop);
    const d = disabled ?? useEnhancementStore.getState().disabledFaces.get(file);

    return d?.size ? faces.filter((_, i) => !d.has(i)) : faces;
};
