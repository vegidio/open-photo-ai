import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import { Kyoto, type Operation } from '@/operations';

type OptionsUpscaleStore = {
    model: Operation;

    setModel: (op: Operation) => void;
};

export const useOptionsUpscaleStore = create(
    immer<OptionsUpscaleStore>((set, _) => ({
        model: new Kyoto('general', 4, 'fp32'),

        setModel: (op: Operation) => {
            set((state) => {
                state.model = op;
            });
        },
    })),
);
