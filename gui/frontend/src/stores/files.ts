import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type FileStore = {
    filePaths: string[];
    selectedFileIndex: number;

    addFilePaths: (filePaths: string[]) => void;
    removeFilePath: (filePath: string) => void;
    clear: () => void;
};

export const useFileStore = create(
    immer<FileStore>((set, get) => ({
        filePaths: [],
        selectedFileIndex: 0,

        addFilePaths: (filePaths: string[]) => {
            set((state) => {
                const uniqueNewPaths = filePaths.filter((path) => !state.filePaths.includes(path));
                state.filePaths = [...state.filePaths, ...uniqueNewPaths];
            });
        },

        removeFilePath: (filePath: string) => {
            set((state) => {
                state.filePaths = state.filePaths.filter((path) => path !== filePath);
            });
        },

        clear: () => {
            set((state) => {
                state.filePaths = [];
            });
        },
    })),
);
