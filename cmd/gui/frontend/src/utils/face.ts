import { LRUCache } from 'lru-cache';
import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { ExecutionProvider, Precision } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import type { CropInfo, File } from '@/bindings/gui/types';
import { DetectFaces } from '@/bindings/gui/services/faceservice.ts';
import { useEnhancementStore } from '@/stores/enhancements.ts';
import { EMPTY_CROP } from '@/utils/constants.ts';
import { cropToken } from '@/utils/crop.ts';

// Caches the in-flight promise, not the resolved faces, so concurrent callers for the same key share one detection
// run. Caching the result instead would let two callers that both miss (the preview and an export of the same file,
// say) each pay a full detection inference.
//
// Bounded, unlike the plain Map this used to be: there is one entry per (file hash, crop) pair, and every adjustment
// of a crop box adds another, so a long editing session grew it without limit. Entries are small — a handful of boxes
// and landmarks — so a plain count bound is enough here, and there is nothing to release on eviction (no object URLs,
// unlike the image cache). Detection is deterministic for a given image and crop, so an evicted entry simply costs
// one more inference if it is ever asked for again.
const facesCache = new LRUCache<string, Promise<Face[]>>({ max: 500 });

/**
 * The precision the detection pass runs at for a set of operations: the one the face-recovery operation itself was
 * built at, so a recovery on the SD tier is fed by the fp16 detection graph and one on HD by the fp32 one. Undefined
 * when there is no face-recovery operation, which is also the answer to "should anything be detected at all".
 *
 * The id layout is the `<prefix>_<name>_<precision>` contract that `operations/factory.ts` owns. The cast is the
 * boundary between the frontend, which carries precisions as the plain strings those ids are built from, and the
 * generated binding, which types them as the Go enum — the two are the same set of strings.
 */
export const faceRecoveryPrecision = (opIds: string[]): Precision | undefined =>
    opIds.find((id) => id.startsWith('fr_'))?.split('_')[2] as Precision | undefined;

/**
 * Detects the faces in an image, caching the result by file hash (plus the precision and a crop token) to avoid
 * redundant detection.
 *
 * Detection runs on the cropped image, so the returned bounding boxes live in the cropped image's coordinate space —
 * matching the cropped source that face recovery and the preview operate on. The resulting faces are passed back to
 * the inference calls (ProcessImage/ExportImage). Faces are deterministic for a given image+crop+precision, so
 * caching by the three is always safe. Omit `crop` to detect against the uncropped image.
 *
 * The precision is in the key because it selects the model file, not because the two disagree about what they find:
 * measured over 25 variants of the sample image, fp16 and fp32 returned the same faces in the same order, differing
 * only sub-pixel. That is what keeps `disabledFaces` — indices into this array — valid across a tier switch.
 */
export const detectFaces = (
    file: File,
    ep: ExecutionProvider,
    precision: Precision,
    crop?: CropInfo,
): Promise<Face[]> => {
    const key = `${file.Hash}_${precision}${cropToken(crop)}`;
    let faces = facesCache.get(key);

    if (!faces) {
        faces = DetectFaces(file.Path, ep, precision, crop ?? EMPTY_CROP).catch((e) => {
            // Only successes are cached: a detection that failed (or was cancelled) must be retried on the next call,
            // not replayed from the cache forever.
            facesCache.delete(key);
            throw e;
        });

        facesCache.set(key, faces);
    }

    return faces;
};

/**
 * Drops every cached detection. The cache is bounded, so this is about correctness rather than memory: closing a
 * workspace should not leave detections from it available to the next one.
 */
export const clearFacesCache = () => {
    facesCache.clear();
};

/**
 * Reports whether any of the given operation IDs is a face-recovery operation (and therefore needs detected faces).
 */
export const hasFaceRecovery = (opIds: string[]): boolean => opIds.some((id) => id.startsWith('fr_'));

/**
 * Resolves the faces an inference run should receive: an empty array when no face-recovery operation is present,
 * otherwise the detected faces (cached by hash) minus any the user has deselected.
 *
 * @param disabled - An already-read disabled-face selection to reuse (e.g. when the caller also needs it for a cache
 *   key); when omitted, it is read from the enhancement store.
 * @param crop - The flip/rotate/crop to detect against; omit for the uncropped image.
 */
export const getEnabledFaces = async (
    file: File,
    ep: ExecutionProvider,
    opIds: string[],
    disabled?: Set<number>,
    crop?: CropInfo,
): Promise<Face[]> => {
    // Subsumes the `hasFaceRecovery` guard this used to open with: no face-recovery operation means no precision to
    // detect at, which is the same "there is nothing to detect for" answer arrived at one step earlier.
    const precision = faceRecoveryPrecision(opIds);
    if (!precision) return [];

    const faces = await detectFaces(file, ep, precision, crop);
    const d = disabled ?? useEnhancementStore.getState().disabledFaces.get(file.Path);

    return d?.size ? faces.filter((_, i) => !d.has(i)) : faces;
};
