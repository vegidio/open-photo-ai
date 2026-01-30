import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

type SettingsStore = {
    processor: string;
    frModel: string;
    laModel: string;
    upModel: string;

    setProcessor: (processor: string) => void;
    setFrModel: (model: string) => void;
    setLaModel: (model: string) => void;
    setUpModel: (model: string) => void;

    saveSnapshot: () => void;
    restoreSnapshot: () => void;
};

export const useSettingsStore = create(
    persist(
        immer<SettingsStore>((set, get) => {
            let snapshot: Record<string, any> = {};

            return {
                processor: 'cpu',
                frModel: 'athens',
                laModel: 'paris',
                upModel: 'kyoto',

                setProcessor: (processor: string) => {
                    set((state) => {
                        state.processor = processor;
                    });
                },

                setFrModel: (model: string) => {
                    set((state) => {
                        state.frModel = model;
                    });
                },

                setLaModel: (model: string) => {
                    set((state) => {
                        state.laModel = model;
                    });
                },

                setUpModel: (model: string) => {
                    set((state) => {
                        state.upModel = model;
                    });
                },

                saveSnapshot: () => {
                    const state = get();
                    snapshot = {};

                    Object.keys(state).forEach((key) => {
                        if (typeof state[key as keyof SettingsStore] !== 'function') {
                            snapshot[key] = state[key as keyof SettingsStore];
                        }
                    });
                },

                restoreSnapshot: () => {
                    set((state) => {
                        Object.keys(snapshot).forEach((key) => {
                            (state as any)[key] = snapshot[key];
                        });
                    });
                },
            };
        }),
        {
            name: 'settings-storage',
        },
    ),
);
