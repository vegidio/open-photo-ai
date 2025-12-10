import { enableMapSet } from 'immer';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { Operation } from '@/operations';

type EnhancementStore = {
    autopilot: boolean;
    operations: Map<string, Operation[]>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addOperation: (filePath: string, operation: Operation) => void;
    removeOperation: (filePath: string, id: string) => void;

    removeFile: (filePath: string) => void;
    clearFiles: () => void;
};

// Enable MapSet support in Immer
enableMapSet();

export const useEnhancementStore = create(
    immer<EnhancementStore>((set, _) => ({
        autopilot: false,
        operations: new Map<string, Operation[]>(),

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

        addOperation: (filePath: string, operation: Operation) => {
            set((state) => {
                const ops = state.operations.get(filePath) ?? [];
                state.operations.set(filePath, [...ops, operation]);
            });
        },

        removeOperation: (filePath: string, id: string) => {
            set((state) => {
                const ops = state.operations.get(filePath) ?? [];
                state.operations.set(
                    filePath,
                    ops.filter((op) => op.id !== id),
                );
            });
        },

        removeFile: (filePath: string) => {
            set((state) => {
                state.operations.delete(filePath);
            });
        },

        clearFiles: () => {
            set((state) => {
                state.operations.clear();
            });
        },
    })),
);
