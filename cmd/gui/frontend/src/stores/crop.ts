import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { CropInfo, File } from '@/bindings/gui/types';

type CropStore = {
    // Per-file flip/rotate/crop applied in the Crop/Rotate modal. Absent = no crop.
    crops: Map<File, CropInfo>;

    setCrop: (file: File, crop: CropInfo) => void;

    removeKey: (file: File) => void;
    clear: () => void;
};

// Enable MapSet support in Immer
enableMapSet();

export const useCropStore = create(
    immer<CropStore>((set, _) => ({
        crops: new Map(),

        setCrop: (file: File, crop: CropInfo) => {
            set((state) => {
                state.crops.set(file, crop);
            });
        },

        removeKey: (file: File) => {
            set((state) => {
                state.crops.delete(file);
            });
        },

        clear: () => {
            set((state) => {
                state.crops.clear();
            });
        },
    })),
);
