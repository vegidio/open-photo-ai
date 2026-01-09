import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '@/bindings/gui/types';
import { useEnhancementStore } from '@/stores/enhancements.ts';
import { useImageStore } from '@/stores/image.ts';

type FileStore = {
    files: File[];
    selectedFiles: File[];
    currentIndex: number;

    setCurrentIndex: (index: number) => void;
    addFiles: (files: File[]) => void;
    removeFile: (file: File) => void;
    addSelectedFile: (file: File) => void;
    removeSelectedFile: (path: string) => void;
    selectAll: () => void;
    unselectAll: () => void;
    clear: () => void;
};

export const useFileStore = create(
    immer<FileStore>((set, get) => ({
        files: [],
        selectedFiles: [],
        currentIndex: 0,

        setCurrentIndex: (index: number) => {
            set((state) => {
                state.currentIndex = index;
            });
        },

        addFiles: (files: File[]) => {
            // Check if the list of files was empty before adding new ones
            const wasEmpty = get().files.length === 0;

            set((state) => {
                const uniqueFiles = files.filter(
                    (file) => !state.files.some((existingFile) => existingFile.Path === file.Path),
                );
                state.files.push(...uniqueFiles);
            });

            // If the list was empty and now has files, select the first one
            if (wasEmpty && get().files.length > 0) {
                get().addSelectedFile(get().files[0]);
            }
        },

        removeFile: (file: File) => {
            set((state) => {
                const removedIndex = state.files.findIndex((f) => f.Path === file.Path);
                if (removedIndex === -1) return;

                state.files = state.files.filter((f) => f.Path !== file.Path);

                // Update currentIndex if necessary
                if (state.files.length === 0) {
                    state.currentIndex = 0;
                } else if (state.currentIndex >= state.files.length) {
                    state.currentIndex = state.files.length - 1;
                }
            });

            // Remove any enhancements and image transforms associated with the removed file
            useEnhancementStore.getState().removeKey(file);
            useImageStore.getState().removeImageTransform(file.Hash);
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

            // Clear all enhancements and image transforms as well
            useEnhancementStore.getState().clear();
            useImageStore.getState().clear();
        },
    })),
);
