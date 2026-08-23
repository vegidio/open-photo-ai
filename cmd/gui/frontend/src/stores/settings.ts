import { persist } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';
import type { SupportedEPs } from '@/bindings/gui/services';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { DEFAULT_LANGUAGE, detectLanguage, isSupportedLanguage, type SupportedLanguage } from '@/i18n/languages';
import { os } from '@/utils/constants';
import { DEFAULT_MODELS, type EnhancementType, type ModelChoices } from '@/utils/enhancement';
import {
    clampQuality,
    DEFAULT_QUALITY,
    normalizeQuality,
    type QualityChoices,
    type QualityFormat,
} from '@/utils/quality';

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

    // The default model for each enhancement, keyed by its two-letter type. One record rather than seven parallel
    // fields, so adding an enhancement is an entry in ENHANCEMENTS and nothing here.
    models: ModelChoices;

    // The encoder quality for each lossy export format. Shared by the Settings dialog and the Export dialog rather
    // than duplicated: the Export dialog's slider is seeded from this, and pressing Export writes back to it, which
    // is what makes the next export of the same format remember the last value used.
    quality: QualityChoices;

    setIsFirstTensorRT: (isFirstRun: boolean) => void;
    setProcessorOptions: (supportedEps: SupportedEPs) => void;
    setExecutionProvider: (ep: ExecutionProvider) => void;
    setAnalyticsEnabled: (enabled: boolean) => void;
    setLanguage: (language: SupportedLanguage) => void;
    setModel: (type: EnhancementType, model: string) => void;
    setQuality: (format: QualityFormat, value: number) => void;

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
    'models',
    'quality',
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
                models: { ...DEFAULT_MODELS },
                quality: { ...DEFAULT_QUALITY },

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

                setModel: (type: EnhancementType, model: string) => {
                    set((state) => {
                        state.models[type] = model;
                    });
                },

                setQuality: (format: QualityFormat, value: number) => {
                    set((state) => {
                        state.quality[format] = clampQuality(value);
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
                        // Arrays and the models record are copied rather than referenced, so editing settings after a
                        // snapshot cannot mutate the snapshot itself and make Cancel a no-op.
                        // biome-ignore lint/suspicious/noExplicitAny: typed key, runtime-safe
                        (saved as any)[key] = Array.isArray(value)
                            ? [...value]
                            : value !== null && typeof value === 'object'
                              ? { ...value }
                              : value;
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
                if (!state) return;
                if (!isSupportedLanguage(state.language)) state.language = DEFAULT_LANGUAGE;

                // Same reasoning again, but the stakes are higher: a missing or out-of-range quality is not a setting
                // that merely looks wrong in the UI - it is handed straight to the native encoders, which take a 0 as
                // a real request and write out a garbage image. See normalizeQuality.
                state.quality = normalizeQuality(state.quality);
            },
        },
    ),
);
