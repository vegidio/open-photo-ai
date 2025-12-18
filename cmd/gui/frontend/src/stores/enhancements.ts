import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { Operation } from '@/operations';
import type { File } from '../../bindings/gui/types';

type EnhancementStore = {
    autopilot: boolean;
    enhancements: Map<File, Operation[]>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addEnhancements: (file: File, operations: Operation[]) => void;
    removeEnhancement: (file: File, id: string) => void;

    removeFile: (file: File) => void;
    clearFiles: () => void;
};

// Enable MapSet support in Immer
enableMapSet();

export const useEnhancementStore = create(
    immer<EnhancementStore>((set, _) => ({
        autopilot: false,
        enhancements: new Map<File, Operation[]>(),

        setAutopilot: (enable: boolean) => {
            set((state) => {
                state.autopilot = enable;
            });
        },

        toggle: () => {
            set((state) => {
                state.autopilot = !state.autopilot;
            });
        },

        addEnhancements: (file: File, operations: Operation[]) => {
            set((state) => {
                const existingOps = state.enhancements.get(file) ?? [];

                // Check if there's already an upscale operation;
                // Upscale operations should always be the last to be processed
                const firstUpscaleIndex = existingOps.findIndex((op) => op.id.startsWith('up'));

                // If there's an upscale operation, insert before it; otherwise add at the end
                if (firstUpscaleIndex !== -1) {
                    const newOps = [
                        ...existingOps.slice(0, firstUpscaleIndex),
                        ...operations,
                        ...existingOps.slice(firstUpscaleIndex),
                    ];
                    state.enhancements.set(file, newOps);
                } else {
                    state.enhancements.set(file, [...existingOps, ...operations]);
                }
            });
        },

        removeEnhancement: (file: File, id: string) => {
            set((state) => {
                const ops = (state.enhancements.get(file) ?? []).filter((op) => op.id !== id);
                state.enhancements.set(file, ops);
            });
        },

        removeFile: (file: File) => {
            set((state) => {
                state.enhancements.delete(file);
            });
        },

        clearFiles: () => {
            set((state) => {
                state.enhancements.clear();
            });
        },
    })),
);
