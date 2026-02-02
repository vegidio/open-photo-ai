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
    isFirstTensorRT: boolean;
    processorSelectItems: SelectItem[];
    executionProvider: ExecutionProvider;

    frModel: string;
    laModel: string;
    upModel: string;

    setIsFirstTensorRT: (isFirstRun: boolean) => void;
    setProcessorSelectItems: (supportedEps: SupportedEPs) => void;
    setExecutionProvider: (ep: ExecutionProvider) => void;
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
                isFirstTensorRT: true,
                processorSelectItems: [],
                executionProvider: ExecutionProviderAuto,
                frModel: 'athens',
                laModel: 'paris',
                upModel: 'kyoto',

                setIsFirstTensorRT: (isFirst: boolean) => {
                    set((state) => {
                        state.isFirstTensorRT = isFirst;
                    });
                },

                setProcessorSelectItems: (supportedEps: SupportedEPs) => {
                    const items: SelectItem[] = [{ label: ExecutionProviderAuto, value: ExecutionProviderAuto }];

                    if (supportedEps.TensorRT)
                        items.push({ label: ExecutionProviderTensorRT, value: ExecutionProviderTensorRT });
                    if (supportedEps.CUDA) items.push({ label: ExecutionProviderCUDA, value: ExecutionProviderCUDA });
                    if (supportedEps.CoreML)
                        items.push({ label: ExecutionProviderCoreML, value: ExecutionProviderCoreML });
                    if (os === 'windows')
                        items.push({ label: ExecutionProviderDirectML, value: ExecutionProviderDirectML });

                    items.push({ label: ExecutionProviderCPU, value: ExecutionProviderCPU });

                    set((state) => {
                        state.processorSelectItems = items;
                    });
                },

                setExecutionProvider: (ep: ExecutionProvider) => {
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
