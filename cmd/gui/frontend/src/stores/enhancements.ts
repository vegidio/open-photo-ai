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

                    // Combine existing and new operations
                    const allOps = [...existingOps, ...operations];

                    // Sort operations by prefix priority: fr -> la -> up
                    const sortedOps = allOps.sort((a, b) => {
                        const getPriority = (op: Operation) => {
                            if (op.id.startsWith('fr')) return 0;
                            if (op.id.startsWith('la')) return 1;
                            if (op.id.startsWith('up')) return 2;
                            return 3; // Any other prefix goes last
                        };

                        return getPriority(a) - getPriority(b);
                    });

                    state.enhancements.set(file, sortedOps);
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
