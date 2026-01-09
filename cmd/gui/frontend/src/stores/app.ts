import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type AppStore = {
    previewMode: 'full' | 'side' | 'split';

    setPreviewMode: (mode: 'full' | 'side' | 'split') => void;
};

export const useAppStore = create(
    persist(
        immer<AppStore>((set, _) => ({
            previewMode: 'side',

            setPreviewMode: (mode: 'full' | 'side' | 'split') => {
                set((state) => {
                    state.previewMode = mode;
                });
            },
        })),
        {
            name: 'app-storage',
        },
    ),
);
