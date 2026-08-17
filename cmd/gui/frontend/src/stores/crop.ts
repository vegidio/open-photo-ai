import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { CropInfo, File } from '@/bindings/gui/types';

type CropStore = {
    // Per-file flip/rotate/crop applied in the Crop/Rotate modal, keyed by `File.Path`. Absent = no crop.
    //
    // Keyed by path rather than by the File object: a Map keyed on the object uses reference identity, so any path
    // that rebuilds a File for the same image (a re-drop, a re-hash) would silently orphan its crop and leave the old
    // entry behind forever. The path is stable and unique within the file list.
    crops: Map<string, CropInfo>;

    setCrop: (file: File, crop: CropInfo) => void;

    removeKey: (file: File) => void;
    clear: () => void;
};

enableMapSet();

export const useCropStore = create(
    immer<CropStore>((set, _) => ({
        crops: new Map(),

        setCrop: (file: File, crop: CropInfo) => {
            set((state) => {
                state.crops.set(file.Path, crop);
            });
        },

        removeKey: (file: File) => {
            set((state) => {
                state.crops.delete(file.Path);
            });
        },

        clear: () => {
            set((state) => {
                state.crops.clear();
            });
        },
    })),
);
