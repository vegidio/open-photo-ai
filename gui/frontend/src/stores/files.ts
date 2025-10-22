import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type FileStore = {
    filePaths: string[];
    selectedIndex: number;
    originalImage?: string;
    enhancedImage?: string;

    addFilePaths: (filePaths: string[]) => void;
    removeFilePath: (filePath: string) => void;
    clear: () => void;
    setOriginalImage: (image: string) => void;
    setEnhancedImage: (image: string) => void;
};

export const useFileStore = create(
    immer<FileStore>((set, get) => ({
        filePaths: [],
        selectedIndex: 0,
        originalImage: undefined,
        enhancedImage: undefined,

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

        setOriginalImage: (image: string) => {
            set((state) => {
                state.originalImage = image;
            });
        },

        setEnhancedImage: (image: string) => {
            set((state) => {
                state.enhancedImage = image;
            });
        },
    })),
);
