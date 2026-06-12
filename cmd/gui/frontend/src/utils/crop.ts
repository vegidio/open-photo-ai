import type { CropInfo } from '@/bindings/gui/types';

/**
 * A stable cache-key fragment for a crop; empty string when there's no crop so uncropped keys stay unchanged.
 *
 * Shared by the image/enhanced-image caches and the face-detection cache so all three key consistently on the crop.
 *
 * @param crop - The crop to serialize, or undefined for no crop.
 */
export const cropToken = (crop?: CropInfo) =>
    crop
        ? `_c${crop.Rotation}-${crop.FlipH ? 1 : 0}${crop.FlipV ? 1 : 0}-${crop.Left}-${crop.Top}-${crop.Width}-${crop.Height}`
        : '';
