import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

// Where a batch export is in its lifecycle. It lives in the store rather than in the component that drives it because
// the dialog above also has to read it - it refuses to close mid-export - and a boolean mirrored up through props was
// a second representation of the same fact, kept in sync by hand.
export type ExportRunState = 'idle' | 'processing' | 'completed';

type ExportStore = {
    key: number;
    runState: ExportRunState;
    prefix: string;
    suffix: string;
    overwrite: boolean;
    location?: string;
    format: string;

    resetKey: () => void;
    setRunState: (runState: ExportRunState) => void;
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
            runState: 'idle',
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

            setRunState: (runState: ExportRunState) => {
                set((state) => {
                    state.runState = runState;
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
                // Store everything, except the fields that only describe this session: `key`, and `runState`, which
                // would otherwise come back as 'processing' after a crash mid-export and lock the dialog shut.
                const { key, runState, ...rest } = state;
                return rest;
            },
        },
    ),
);
