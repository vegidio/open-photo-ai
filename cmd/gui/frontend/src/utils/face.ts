import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import type { File } from '@/bindings/gui/types';
import { DetectFaces } from '@/bindings/gui/services/faceservice.ts';
import { useEnhancementStore } from '@/stores/enhancements.ts';

const facesCache = new Map<string, Face[]>();

/**
 * Detects the faces in an image, caching the result by file hash to avoid redundant detection.
 *
 * Face detection runs independently of face recovery: the resulting faces are passed back to the inference calls
 * (ProcessImage/ExportImage). Faces are deterministic for a given image, so caching by hash is always safe.
 *
 * @param file - The file object containing the image path and hash.
 * @param ep - The execution provider to use for detection.
 * @returns A promise that resolves to the detected faces (empty when none are found).
 */
export const detectFaces = async (file: File, ep: ExecutionProvider): Promise<Face[]> => {
    let faces = facesCache.get(file.Hash);

    if (!faces) {
        faces = await DetectFaces(file.Path, ep);
        facesCache.set(file.Hash, faces);
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
 * @returns A promise that resolves to the enabled faces to pass to the inference call.
 */
export const getEnabledFaces = async (
    file: File,
    ep: ExecutionProvider,
    opIds: string[],
    disabled?: Set<number>,
): Promise<Face[]> => {
    if (!hasFaceRecovery(opIds)) return [];

    const faces = await detectFaces(file, ep);
    const d = disabled ?? useEnhancementStore.getState().disabledFaces.get(file);

    return d?.size ? faces.filter((_, i) => !d.has(i)) : faces;
};
