import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '@/bindings/gui/types';

enableMapSet();

type FileStore = {
    files: File[];
    selectedFiles: File[];

    // The paths in selectedFiles, kept alongside it purely so a membership test is O(1).
    //
    // Zustand re-runs every subscriber's selector on every store write, and each drawer item subscribes with its own
    // "am I selected?" check. Scanning selectedFiles made one "select all" cost O(n^2) comparisons on the click's
    // synchronous path - a quarter of a million of them at 500 files. Every writer below updates both.
    selectedPaths: Set<string>;

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
        selectedPaths: new Set<string>(),
        currentIndex: 0,

        setCurrentIndex: (index: number) => {
            set((state) => {
                state.currentIndex = index;
            });
        },

        addFiles: (files: File[]) => {
            const oldLength = get().files.length;
            const wasEmpty = oldLength === 0;

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

                if (state.selectedPaths.delete(file.Path)) {
                    state.selectedFiles = state.selectedFiles.filter((f) => f.Path !== file.Path);
                }

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
                if (state.selectedPaths.has(file.Path)) return;

                state.selectedFiles.push(file);
                state.selectedPaths.add(file.Path);
            });
        },

        removeSelectedFile: (path: string) => {
            set((state) => {
                if (!state.selectedPaths.delete(path)) return;

                state.selectedFiles = state.selectedFiles.filter((file) => file.Path !== path);
            });
        },

        selectAll: () => {
            set((state) => {
                // A copy, not an alias. Assigning state.files directly left both fields pointing at one array, so
                // "these are two independent lists" quietly stopped being true.
                state.selectedFiles = [...state.files];
                state.selectedPaths = new Set(state.files.map((file) => file.Path));
            });
        },

        unselectAll: () => {
            set((state) => {
                state.selectedFiles = [];
                state.selectedPaths = new Set();
            });
        },

        clear: () => {
            set((state) => {
                state.files = [];
                state.selectedFiles = [];
                state.selectedPaths = new Set();
                state.currentIndex = 0;
            });
        },
    })),
);
