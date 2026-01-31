import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { SupportedEPs } from '@/bindings/gui/services';
import type { SelectItem } from '@/components/atoms/Select';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { os } from '@/utils/constants';

const {
    ExecutionProviderCUDA,
    ExecutionProviderDirectML,
    ExecutionProviderTensorRT,
    ExecutionProviderCoreML,
    ExecutionProviderAuto,
    ExecutionProviderCPU,
} = ExecutionProvider;

type SettingsStore = {
    isFirstRun: boolean;
    processorSelectItems: SelectItem[];
    executionProvider: ExecutionProvider;

    frModel: string;
    laModel: string;
    upModel: string;

    setIsFirstRun: (isFirstRun: boolean) => void;
    setProcessorSelectItems: (supportedEps: SupportedEPs) => void;
    setExecutionProvider: (epStr: string) => void;
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
                isFirstRun: true,
                processorSelectItems: [],
                executionProvider: ExecutionProviderAuto,
                frModel: 'athens',
                laModel: 'paris',
                upModel: 'kyoto',

                setIsFirstRun: (isFirstRun: boolean) => {
                    set((state) => {
                        state.isFirstRun = isFirstRun;
                    });
                },

                setProcessorSelectItems: (supportedEps: SupportedEPs) => {
                    const items: SelectItem[] = [{ label: 'Auto', value: ExecutionProviderAuto.toString() }];

                    if (supportedEps.TensorRT)
                        items.push({ label: 'TensorRT', value: ExecutionProviderTensorRT.toString() });
                    if (supportedEps.CUDA) items.push({ label: 'CUDA', value: ExecutionProviderCUDA.toString() });
                    if (supportedEps.CoreML) items.push({ label: 'CoreML', value: ExecutionProviderCoreML.toString() });
                    if (os === 'windows')
                        items.push({ label: 'DirectML', value: ExecutionProviderDirectML.toString() });

                    items.push({ label: 'CPU', value: ExecutionProviderCPU.toString() });

                    set((state) => {
                        state.processorSelectItems = items;
                    });
                },

                setExecutionProvider: (epStr: string) => {
                    const ep: ExecutionProvider = parseInt(epStr, 10);

                    set((state) => {
                        state.executionProvider = ep;
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
