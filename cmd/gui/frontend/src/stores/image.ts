import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { ImageData } from '@/utils/image.ts';

export type ImageTransform = {
    scale: number;
    positionX: number;
    positionY: number;
    // Point of the displayed image to keep fixed while the scale changes, as fractions [0..1].
    // Set by the mouse wheel/pinch zoom (the point under the cursor) and omitted by the drawer
    // slider and +/- buttons, which stay anchored to the center of the container.
    anchor?: { x: number; y: number };
};

// Portion of the image currently visible in the Preview, as fractions [0..1] of the displayed image
export type ImageViewport = {
    x: number;
    y: number;
    width: number;
    height: number;
};

// The viewport is written from a pan handler, so it is compared before it is stored - see setViewport.
const sameViewport = (a: ImageViewport | undefined, b: ImageViewport | undefined): boolean => {
    if (a === b) return true;
    if (!a || !b) return false;

    return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
};

enableMapSet();

type ImageStore = {
    originalImage?: ImageData;
    enhancedImage?: ImageData;
    imageTransform: Map<string, ImageTransform>;
    viewport?: ImageViewport;

    setOriginalImage: (image: ImageData | undefined) => void;
    setEnhancedImage: (image: ImageData | undefined) => void;
    setImageTransform: (id: string, imageState: ImageTransform) => void;
    setViewport: (viewport: ImageViewport | undefined) => void;

    removeImageTransform: (id: string) => void;
    clear: () => void;
};

export const useImageStore = create(
    immer<ImageStore>((set, get) => ({
        originalImage: undefined,
        enhancedImage: undefined,
        imageTransform: new Map(),
        viewport: undefined,

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

        // Compared before storing, unlike the setters above. In "side" and "split" mode two ZoomImage panes share one
        // transform key and both recompute this single slot on every animation frame of a pan, each writing a fresh
        // object. Without the comparison every one of those frames re-rendered each viewport subscriber - SidebarImage
        // draws from it - with numbers that were usually identical to the ones already there.
        setViewport: (viewport: ImageViewport | undefined) => {
            if (sameViewport(get().viewport, viewport)) return;

            set((state) => {
                state.viewport = viewport;
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
                state.viewport = undefined;
            });
        },
    })),
);
