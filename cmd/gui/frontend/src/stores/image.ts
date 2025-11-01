import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { ImageData } from '@/utils/image.ts';

type ImageStore = {
    running: boolean;
    originalImage?: ImageData;
    enhancedImage?: ImageData;

    setIsRunning: (running: boolean) => void;
    setOriginalImage: (image: ImageData | undefined) => void;
    setEnhancedImage: (image: ImageData | undefined) => void;
};

export const useImageStore = create(
    immer<ImageStore>((set, _) => ({
        running: false,
        originalImage: undefined,
        enhancedImage: undefined,

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
    })),
);
