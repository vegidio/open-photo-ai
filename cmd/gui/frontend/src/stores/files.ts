import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '../../bindings/gui/types';

type FileStore = {
    files: File[];
    selectedIndex: number;

    setSelectedIndex: (index: number) => void;
    addFiles: (files: File[]) => void;
    removeFile: (hash: string) => void;
    clear: () => void;
};

export const useFileStore = create(
    immer<FileStore>((set, _) => ({
        files: [],
        selectedIndex: 0,

        setSelectedIndex: (index: number) => {
            set((state) => {
                state.selectedIndex = index;
            });
        },

        addFiles: (files: File[]) => {
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
    })),
);
