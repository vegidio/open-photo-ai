import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type ExportStore = {
    key: number;
    prefix: string;
    suffix: string;
    overwrite: boolean;
    location?: string;
    format: string;

    resetKey: () => void;
    setPrefix: (prefix: string) => void;
    setSuffix: (suffix: string) => void;
    setOverwrite: (overwrite: boolean) => void;
    setLocation: (location?: string) => void;
    setFormat: (format: string) => void;
};

export const useExportStore = create(
    persist(
        immer<ExportStore>((set, _) => ({
            key: Date.now(),
            prefix: '',
            suffix: '',
            overwrite: false,
            location: undefined,
            format: 'png',

            resetKey: () => {
                set((state) => {
                    state.key = Date.now();
                });
            },

            setPrefix: (prefix: string) => {
                set((state) => {
                    state.prefix = prefix;
                });
            },

            setSuffix: (suffix: string) => {
                set((state) => {
                    state.suffix = suffix;
                });
            },

            setOverwrite: (overwrite: boolean) => {
                set((state) => {
                    state.overwrite = overwrite;
                });
            },

            setLocation: (location?: string) => {
                set((state) => {
                    state.location = location;
                });
            },

            setFormat: (format: string) => {
                set((state) => {
                    state.format = format;
                });
            },
        })),
        {
            name: 'export-storage',
            partialize: (state) => {
                // biome-ignore lint/correctness/noUnusedVariables: Store everything, except the `key` field
                const { key, ...rest } = state;
                return rest;
            },
        },
    ),
);
