import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type FileListStore = {
    open: boolean;

    setOpen: (open: boolean) => void;
    toggle: () => void;
};

export const useFileListStore = create(
    immer<FileListStore>((set, _) => ({
        open: false,

        setOpen: (open: boolean) => {
            set((state) => {
                state.open = open;
            });
        },

        toggle: () => {
            set((state) => {
                state.open = !state.open;
            });
        },
    })),
);
