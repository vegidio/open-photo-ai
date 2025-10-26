import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type ImageStore = {
    running: boolean;
    originalImage?: string;
    enhancedImage?: string;

    setIsRunning: (running: boolean) => void;
    setOriginalImage: (image: string | undefined) => void;
    setEnhancedImage: (image: string | undefined) => void;
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

        setOriginalImage: (image: string | undefined) => {
            set((state) => {
                state.originalImage = image;
            });
        },

        setEnhancedImage: (image: string | undefined) => {
            set((state) => {
                state.enhancedImage = image;
            });
        },
    })),
);
