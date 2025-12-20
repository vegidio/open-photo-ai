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
    running: boolean;
    originalImage?: ImageData;
    enhancedImage?: ImageData;
    imageTransform: Map<string, ImageTransform>;

    setIsRunning: (running: boolean) => void;
    setOriginalImage: (image: ImageData | undefined) => void;
    setEnhancedImage: (image: ImageData | undefined) => void;
    setImageTransform: (id: string, imageState: ImageTransform) => void;
};

export const useImageStore = create(
    immer<ImageStore>((set, _) => ({
        running: false,
        originalImage: undefined,
        enhancedImage: undefined,
        imageTransform: new Map(),

        setIsRunning: (running: boolean) => {
            set((state) => {
                state.running = running;
            });
        },

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
    })),
);
