import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { DialogFile } from '../../bindings/gui/types';

type FileStore = {
    files: DialogFile[];
    selectedIndex: number;
    originalImage?: string;
    enhancedImage?: string;

    setSelectedIndex: (index: number) => void;
    addFiles: (files: DialogFile[]) => void;
    removeFile: (hash: string) => void;
    clear: () => void;
    setOriginalImage: (image: string | undefined) => void;
    setEnhancedImage: (image: string | undefined) => void;
};

export const useFileStore = create(
    immer<FileStore>((set, _) => ({
        files: [],
        selectedIndex: 0,
        originalImage: undefined,
        enhancedImage: undefined,

        setSelectedIndex: (index: number) => {
            set((state) => {
                state.selectedIndex = index;
            });
        },

        addFiles: (files: DialogFile[]) => {
            set((state) => {
                const uniqueFiles = files.filter(
                    (file) => !state.files.some((existingFile) => existingFile.Hash === file.Hash),
                );
                state.files.push(...uniqueFiles);
            });
        },

        removeFile: (hash: string) => {
            set((state) => {
                const removedIndex = state.files.findIndex((file) => file.Hash === hash);
                if (removedIndex === -1) return;

                state.files = state.files.filter((file) => file.Hash !== hash);

                // Update selectedIndex if necessary
                if (state.files.length === 0) {
                    state.selectedIndex = 0;
                } else if (state.selectedIndex >= state.files.length) {
                    state.selectedIndex = state.files.length - 1;
                }
            });
        },

        clear: () => {
            set((state) => {
                state.files = [];
                state.selectedIndex = 0;
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
