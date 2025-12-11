import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { Operation } from '@/operations';
import type { DialogFile } from '../../bindings/gui/types';

type EnhancementStore = {
    autopilot: boolean;
    enhancements: Map<DialogFile, Operation[]>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addEnhancement: (file: DialogFile, operation: Operation) => void;
    removeEnhancement: (file: DialogFile, id: string) => void;

    removeFile: (file: DialogFile) => void;
    clearFiles: () => void;
};

// Enable MapSet support in Immer
enableMapSet();

export const useEnhancementStore = create(
    immer<EnhancementStore>((set, _) => ({
        autopilot: false,
        enhancements: new Map<DialogFile, Operation[]>(),

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

        addEnhancement: (file: DialogFile, operation: Operation) => {
            set((state) => {
                const ops = state.enhancements.get(file) ?? [];
                state.enhancements.set(file, [...ops, operation]);
            });
        },

        removeEnhancement: (file: DialogFile, id: string) => {
            set((state) => {
                const ops = (state.enhancements.get(file) ?? []).filter((op) => op.id !== id);

                if (ops.length > 0) {
                    state.enhancements.set(file, ops);
                } else {
                    state.enhancements.delete(file);
                }
            });
        },

        removeFile: (file: DialogFile) => {
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
