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
                state.files = [...state.files, ...uniqueFiles];
            });
        },

        removeFile: (hash: string) => {
            set((state) => {
                state.files = state.files.filter((file) => file.Hash !== hash);
            });
        },

        clear: () => {
            set((state) => {
                state.files = [];
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
