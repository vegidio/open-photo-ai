import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type FileListStore = {
    open: boolean;
    setOpen: (open: boolean) => void;
};

export const useFileListStore = create(
    immer<FileListStore>((set, get) => ({
        open: false,

        setOpen: (open: boolean) => {
            set((state) => {
                state.open = open;
            });
        },
    })),
);
