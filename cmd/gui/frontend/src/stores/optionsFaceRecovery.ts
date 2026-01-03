import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import { Athens, type Operation } from '@/operations';

type OptionsFaceRecoveryStore = {
    model: Operation;

    setModel: (op: Operation) => void;
};

export const useOptionsFaceRecoveryStore = create(
    immer<OptionsFaceRecoveryStore>((set, _) => ({
        model: new Athens('fp32'),

        setModel: (op: Operation) => {
            set((state) => {
                state.model = op;
            });
        },
    })),
);
