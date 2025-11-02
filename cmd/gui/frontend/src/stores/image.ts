import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { ImageData } from '@/utils/image.ts';

type ImageState = {
    scale: number;
    positionX: number;
    positionY: number;
};

type ImageStore = {
    running: boolean;
    originalImage?: ImageData;
    enhancedImage?: ImageData;
    imageState?: ImageState;

    setIsRunning: (running: boolean) => void;
    setOriginalImage: (image: ImageData | undefined) => void;
    setEnhancedImage: (image: ImageData | undefined) => void;
    setImageState: (imageState: ImageState | undefined) => void;
};

export const useImageStore = create(
    immer<ImageStore>((set, _) => ({
        running: false,
        originalImage: undefined,
        enhancedImage: undefined,
        position: { scale: 1, positionX: 0, positionY: 0 },

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

        setImageState: (imageState: ImageState | undefined) => {
            set((state) => {
                state.imageState = imageState;
            });
        },
    })),
);
