import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type ControlStore = {
    autopilot: boolean;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;
};

export const useControlStore = create(
    immer<ControlStore>((set, _) => ({
        autopilot: true,

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
    })),
);
