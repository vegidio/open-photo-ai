import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type DrawerStore = {
    open: boolean;
    zoom: number;

    setOpen: (open: boolean) => void;
    toggle: () => void;
    setZoom: (zoom: number) => void;
};

export const useDrawerStore = create(
    immer<DrawerStore>((set, _) => ({
        open: false,
        zoom: 1,

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

        setZoom: (zoom: number) => {
            set((state) => {
                state.zoom = zoom;
            });
        },
    })),
);
