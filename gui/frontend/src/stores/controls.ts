import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type ControlStore = {
    autopilot: boolean;
    setAutopilot: (enable: boolean) => void;
};

export const useControlStore = create(
    immer<ControlStore>((set, get) => ({
        autopilot: true,

        setAutopilot: (enable: boolean) => {
            set((state) => {
                state.autopilot = enable;
            });
        },
    })),
);
