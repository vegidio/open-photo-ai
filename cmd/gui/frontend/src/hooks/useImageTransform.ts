import type { File } from '@/bindings/gui/types';
import { type ImageTransform, useImageStore } from '@/stores/image.ts';

export const INITIAL_TRANSFORM: ImageTransform = { scale: 1, positionX: 0, positionY: 0 };

export const useImageTransform = (file: File | undefined) =>
    useImageStore((state) => state.imageTransform.get(file?.Hash ?? '')) ?? INITIAL_TRANSFORM;
