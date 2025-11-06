import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { Operation } from '@/operations';

type EnhancementStore = {
    autopilot: boolean;
    operations: Operation[];

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
    addOperation: (operation: Operation) => void;
    removeOperation: (id: string) => void;
};

export const useEnhancementStore = create(
    immer<EnhancementStore>((set, _) => ({
        autopilot: false,
        operations: [],

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

        addOperation: (operation: Operation) => {
            set((state) => {
                state.operations.push(operation);
            });
        },

        removeOperation: (id: string) => {
            set((state) => {
                state.operations = state.operations.filter((operation) => operation.id !== id);
            });
        },
    })),
);
