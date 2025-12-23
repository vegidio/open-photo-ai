import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { ImageData } from '@/utils/image.ts';

export type ImageTransform = {
    scale: number;
    positionX: number;
    positionY: number;
};

// Enable MapSet support in Immer
enableMapSet();

type ImageStore = {
    originalImage?: ImageData;
    enhancedImage?: ImageData;
    imageTransform: Map<string, ImageTransform>;

    setOriginalImage: (image: ImageData | undefined) => void;
    setEnhancedImage: (image: ImageData | undefined) => void;
    setImageTransform: (id: string, imageState: ImageTransform) => void;

    removeImageTransform: (id: string) => void;
    clear: () => void;
};

export const useImageStore = create(
    immer<ImageStore>((set, _) => ({
        originalImage: undefined,
        enhancedImage: undefined,
        imageTransform: new Map(),

        setOriginalImage: (image: ImageData | undefined) => {
            set((state) => {
                state.originalImage = image;
            });
        },

        setEnhancedImage: (image: ImageData | undefined) => {
            set((state) => {
                state.enhancedImage = image;
            });
        },

        setImageTransform: (id: string, imageTransform: ImageTransform) => {
            set((state) => {
                state.imageTransform.set(id, imageTransform);
            });
        },

        removeImageTransform: (id: string) => {
            set((state) => {
                state.imageTransform.delete(id);
            });
        },

        clear: () => {
            set((state) => {
                state.imageTransform.clear();
            });
        },
    })),
);
