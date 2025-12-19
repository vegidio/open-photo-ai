import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type ExportStore = {
    prefix: string;
    suffix: string;
    overwrite: boolean;
    location?: string;
    format: string;

    setPrefix: (prefix: string) => void;
    setSuffix: (suffix: string) => void;
    setOverwrite: (overwrite: boolean) => void;
    setLocation: (location?: string) => void;
    setFormat: (format: string) => void;
};

export const useExportStore = create(
    persist(
        immer<ExportStore>((set, _) => ({
            prefix: '',
            suffix: '',
            overwrite: false,
            location: undefined,
            format: 'png',

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
        },
    ),
);
