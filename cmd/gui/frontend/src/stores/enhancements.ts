import { enableMapSet } from 'immer';
import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { Face } from '@/bindings/github.com/vegidio/open-photo-ai/models/detection';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';

type EnhancementStore = {
    autopilot: boolean;
    enhancements: Map<File, Operation[]>;
    faces: Map<File, Face[]>;
    // Indices (into `faces`) of the faces the user has disabled (deselected) per file. Absent/empty = all enabled.
    disabledFaces: Map<File, Set<number>>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addEnhancements: (file: File, operations: Operation[]) => void;
    replaceEnhancement: (file: File, operation: Operation) => void;
    removeEnhancement: (file: File, operationId: string) => void;
    setFaces: (file: File, faces: Face[]) => void;
    setDisabledFaces: (file: File, disabled: Set<number>) => void;

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
            faces: new Map(),
            disabledFaces: new Map(),

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

                    // Sort operations by prefix priority: dn -> fr -> la -> cb -> up
                    const sortedOps = allOps.sort((a, b) => {
                        const getPriority = (op: Operation) => {
                            if (op.id.startsWith('dn')) return 0;
                            if (op.id.startsWith('fr')) return 1;
                            if (op.id.startsWith('la')) return 2;
                            if (op.id.startsWith('cb')) return 3;
                            if (op.id.startsWith('up')) return 4;
                            return 5; // Any other prefix goes last
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

                    // Detected faces only matter while a face-recovery op exists; drop them when it's removed.
                    if (operationId.startsWith('fr')) {
                        state.faces.delete(file);
                        state.disabledFaces.delete(file);
                    }
                });
            },

            setFaces: (file: File, faces: Face[]) => {
                set((state) => {
                    state.faces.set(file, faces);
                    // Fresh detection re-numbers faces, so any prior selection is stale — reset to all-enabled.
                    state.disabledFaces.delete(file);
                });
            },

            setDisabledFaces: (file: File, disabled: Set<number>) => {
                set((state) => {
                    state.disabledFaces.set(file, disabled);
                });
            },

            removeKey: (file: File) => {
                set((state) => {
                    state.enhancements.delete(file);
                    state.faces.delete(file);
                    state.disabledFaces.delete(file);
                });
            },

            clear: () => {
                set((state) => {
                    state.enhancements.clear();
                    state.faces.clear();
                    state.disabledFaces.clear();
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
