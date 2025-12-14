import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '../../bindings/gui/types';

type FileStore = {
    files: File[];
    selectedFiles: File[];
    currentIndex: number;

    setCurrentIndex: (index: number) => void;
    addFiles: (files: File[]) => void;
    removeFile: (path: string) => void;
    addSelectedFile: (file: File) => void;
    removeSelectedFile: (path: string) => void;
    selectAll: () => void;
    unselectAll: () => void;
    clear: () => void;
};

export const useFileStore = create(
    immer<FileStore>((set, _) => ({
        files: [],
        selectedFiles: [],
        currentIndex: 0,

        setCurrentIndex: (index: number) => {
            set((state) => {
                state.currentIndex = index;
            });
        },

        addFiles: (files: File[]) => {
            set((state) => {
                const uniqueFiles = files.filter(
                    (file) => !state.files.some((existingFile) => existingFile.Path === file.Path),
                );
                state.files.push(...uniqueFiles);
            });
        },

        removeFile: (path: string) => {
            set((state) => {
                const removedIndex = state.files.findIndex((file) => file.Path === path);
                if (removedIndex === -1) return;

                state.files = state.files.filter((file) => file.Path !== path);

                // Update currentIndex if necessary
                if (state.files.length === 0) {
                    state.currentIndex = 0;
                } else if (state.currentIndex >= state.files.length) {
                    state.currentIndex = state.files.length - 1;
                }
            });
        },

        addSelectedFile: (file: File) => {
            set((state) => {
                const exists = state.selectedFiles.some((existingFile) => existingFile.Path === file.Path);
                if (!exists) state.selectedFiles.push(file);
            });
        },

        removeSelectedFile: (path: string) => {
            set((state) => {
                const removedIndex = state.selectedFiles.findIndex((file) => file.Path === path);
                if (removedIndex === -1) return;

                state.selectedFiles = state.selectedFiles.filter((file) => file.Path !== path);
            });
        },

        selectAll: () => {
            set((state) => {
                state.selectedFiles = state.files;
            });
        },

        unselectAll: () => {
            set((state) => {
                state.selectedFiles = [];
            });
        },

        clear: () => {
            set((state) => {
                state.files = [];
                state.selectedFiles = [];
                state.currentIndex = 0;
            });
        },
    })),
);
