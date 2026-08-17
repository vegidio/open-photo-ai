import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '@/bindings/gui/types';

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
            const wasEmpty = get().files.length === 0;
            const oldLength = get().files.length;

            set((state) => {
                // A Set of the existing paths, rather than a scan per incoming file: dropping a folder onto a long
                // list was O(n·m). Adding to the set as we go also de-duplicates within the incoming batch, which the
                // previous check missed.
                const seenPaths = new Set(state.files.map((existingFile) => existingFile.Path));
                const uniqueFiles = files.filter((file) => {
                    if (seenPaths.has(file.Path)) return false;
                    seenPaths.add(file.Path);
                    return true;
                });

                state.files.push(...uniqueFiles);

                // Move the current selection to the first newly added file
                if (state.files.length > oldLength) {
                    state.currentIndex = oldLength;
                }
            });

            if (wasEmpty && get().files.length > 0) {
                get().addSelectedFile(get().files[0]);
            }
        },

        removeFile: (file: File) => {
            set((state) => {
                const removedIndex = state.files.findIndex((f) => f.Path === file.Path);
                if (removedIndex === -1) return;

                state.files = state.files.filter((f) => f.Path !== file.Path);

                if (state.files.length === 0) {
                    state.currentIndex = 0;
                } else if (removedIndex < state.currentIndex) {
                    state.currentIndex -= 1;
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
