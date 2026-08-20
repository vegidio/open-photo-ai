import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { SupportedEPs } from '@/bindings/gui/services';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { DEFAULT_LANGUAGE, detectLanguage, isSupportedLanguage, type SupportedLanguage } from '@/i18n/languages';
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
    processorOptions: ExecutionProvider[];
    executionProvider: ExecutionProvider;
    analyticsEnabled: boolean;
    language: SupportedLanguage;

    dnModel: string;
    frModel: string;
    laModel: string;
    cbModel: string;
    upModel: string;
    shModel: string;

    setIsFirstTensorRT: (isFirstRun: boolean) => void;
    setProcessorOptions: (supportedEps: SupportedEPs) => void;
    setExecutionProvider: (ep: ExecutionProvider) => void;
    setAnalyticsEnabled: (enabled: boolean) => void;
    setLanguage: (language: SupportedLanguage) => void;
    setDnModel: (model: string) => void;
    setFrModel: (model: string) => void;
    setLaModel: (model: string) => void;
    setCbModel: (model: string) => void;
    setUpModel: (model: string) => void;
    setShModel: (model: string) => void;

    saveSnapshot: () => void;
    restoreSnapshot: () => void;
};

// Keys of SettingsStore that hold data (not actions). Enumerated explicitly so the snapshot is
// compile-time-safe: adding a new data field or renaming one forces this list to update.
const SNAPSHOT_KEYS = [
    'isFirstTensorRT',
    'processorOptions',
    'executionProvider',
    'analyticsEnabled',
    'language',
    'dnModel',
    'frModel',
    'laModel',
    'cbModel',
    'upModel',
    'shModel',
] as const satisfies readonly (keyof SettingsStore)[];

type SnapshotKey = (typeof SNAPSHOT_KEYS)[number];
type SettingsSnapshot = Pick<SettingsStore, SnapshotKey>;

export const useSettingsStore = create(
    persist(
        immer<SettingsStore>((set, get) => {
            let snapshot: SettingsSnapshot | undefined;

            return {
                isFirstTensorRT: true,
                processorOptions: [],
                executionProvider: ExecutionProviderAuto,
                analyticsEnabled: true,
                // Only used on first launch; `persist` replaces it with the stored choice on every later boot.
                language: detectLanguage(),
                dnModel: 'stockholm',
                frModel: 'athens',
                laModel: 'paris',
                cbModel: 'rio',
                upModel: 'kyoto',
                shModel: 'moscow',

                setIsFirstTensorRT: (isFirst: boolean) => {
                    set((state) => {
                        state.isFirstTensorRT = isFirst;
                    });
                },

                // Holds values only, never display labels: the store is persisted, and a label baked into localStorage
                // would keep showing the language it was written in. The Settings item builds the labels at render.
                setProcessorOptions: (supportedEps: SupportedEPs) => {
                    const options: ExecutionProvider[] = [ExecutionProviderAuto];

                    if (supportedEps.TensorRT) options.push(ExecutionProviderTensorRT);
                    if (supportedEps.CUDA) options.push(ExecutionProviderCUDA);
                    if (supportedEps.CoreML) options.push(ExecutionProviderCoreML);
                    if (os === 'windows') options.push(ExecutionProviderDirectML);

                    options.push(ExecutionProviderCPU);

                    set((state) => {
                        state.processorOptions = options;

                        // The chosen processor is persisted, but the hardware behind it may be gone on the next run
                        // (a GPU driver that broke or was uninstalled, a machine the settings were copied to). Without
                        // this the app would keep asking for a processor that is no longer offered — and no longer
                        // works — making every enhancement fail.
                        if (!options.includes(state.executionProvider)) {
                            state.executionProvider = ExecutionProviderAuto;
                        }
                    });
                },

                setExecutionProvider: (ep: ExecutionProvider) => {
                    set((state) => {
                        state.executionProvider = ep;
                    });
                },

                setAnalyticsEnabled: (enabled: boolean) => {
                    set((state) => {
                        state.analyticsEnabled = enabled;
                    });
                },

                // Only writes the store. i18n.changeLanguage() is deliberately left to the Settings dialog's Save
                // handler: the language must apply on Save, and Cancel's restoreSnapshot() has to be able to undo the
                // choice without anything outside the store having observed it.
                setLanguage: (language: SupportedLanguage) => {
                    set((state) => {
                        state.language = language;
                    });
                },

                setDnModel: (model: string) => {
                    set((state) => {
                        state.dnModel = model;
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

                setCbModel: (model: string) => {
                    set((state) => {
                        state.cbModel = model;
                    });
                },

                setUpModel: (model: string) => {
                    set((state) => {
                        state.upModel = model;
                    });
                },

                setShModel: (model: string) => {
                    set((state) => {
                        state.shModel = model;
                    });
                },

                // Both directions iterate SNAPSHOT_KEYS, which is the whole point of that list: a new or renamed
                // settings field updates one place and stays covered, instead of being silently left out of the
                // snapshot until someone notices Cancel doesn't revert it.
                saveSnapshot: () => {
                    const state = get();
                    const saved = {} as SettingsSnapshot;

                    for (const key of SNAPSHOT_KEYS) {
                        const value = state[key];
                        // biome-ignore lint/suspicious/noExplicitAny: typed key, runtime-safe
                        (saved as any)[key] = Array.isArray(value) ? [...value] : value;
                    }

                    snapshot = saved;
                },

                restoreSnapshot: () => {
                    if (!snapshot) return;
                    const saved = snapshot;
                    set((state) => {
                        for (const key of SNAPSHOT_KEYS) {
                            // biome-ignore lint/suspicious/noExplicitAny: typed key, runtime-safe
                            (state as any)[key] = saved[key];
                        }
                    });
                },
            };
        }),
        {
            name: 'settings-storage',

            // Same reasoning as the executionProvider guard above: a language persisted by an older build whose
            // catalog no longer ships (or a hand-edited value) would leave the Settings select on an option that
            // isn't in its item list, and every t() call falling back key-by-key.
            onRehydrateStorage: () => (state) => {
                if (state && !isSupportedLanguage(state.language)) state.language = DEFAULT_LANGUAGE;
            },
        },
    ),
);
