import { enableMapSet } from 'immer';
import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';

type EnhancementStore = {
    autopilot: boolean;
    enhancements: Map<File, Operation[]>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addEnhancements: (file: File, operations: Operation[]) => void;
    replaceEnhancement: (file: File, operation: Operation) => void;
    removeEnhancement: (file: File, operationId: string) => void;

    removeKey: (file: File) => void;
    clear: () => void;
};

// Enable MapSet support in Immer
enableMapSet();

export const useEnhancementStore = create(
    persist(
        immer<EnhancementStore>((set, _) => ({
            autopilot: true,
            enhancements: new Map(),

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

            replaceEnhancement: (file: File, operation: Operation) => {
                set((state) => {
                    const prefix = operation.id.split('_')[0];
                    const ops = (state.enhancements.get(file) ?? []).map((op) =>
                        op.id.startsWith(prefix) ? operation : op,
                    );

                    state.enhancements.set(file, ops);
                });
            },

            removeEnhancement: (file: File, operationId: string) => {
                set((state) => {
                    const ops = (state.enhancements.get(file) ?? []).filter((op) => op.id !== operationId);
                    state.enhancements.set(file, ops);
                });
            },

            removeKey: (file: File) => {
                set((state) => {
                    state.enhancements.delete(file);
                });
            },

            clear: () => {
                set((state) => {
                    state.enhancements.clear();
                });
            },
        })),
        {
            name: 'enhancements-storage',
            partialize: (state) => ({
                // Persist only the `autopilot` state
                autopilot: state.autopilot,
            }),
        },
    ),
);
