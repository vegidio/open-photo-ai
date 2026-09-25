import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { DEFAULT_LANGUAGE, detectLanguage, isLanguage, type Language } from "@/i18n/languages";
import type { Family } from "@/ipc/catalogue";
import type { ExportFormat } from "@/ipc/export";
import type { SupportedProviders } from "@/ipc/setup";
import { familiesWhere } from "@/lib/enhancements";
import { isFormatChoice } from "@/lib/export";
import { clampTo } from "@/lib/utils";

// `"auto"` plus the wire's own field names rather than a list of five strings written out here: the
// four are `keyof SupportedProviders`, so a provider the library adds and `ipc/setup.ts` names
// arrives in this type without this file being edited, and one renamed on the wire is a type error
// rather than an option that quietly stops matching.
//
// The values are the wire's spellings (`tensorrt`), not the Wails application's enum (`"TensorRT"`),
// even though the storage key below is shared with it: this codebase already spells them this way in
// `ipc/setup.ts`, and two spellings of one provider is the drift everything else here is spent
// avoiding. A `settings-storage` written by that application degrades rather than breaks - its
// processor is simply not one of `PROCESSORS`, so it is replaced with the automatic choice, by the
// same rule that exists for a driver that was uninstalled.
/** The processor a user can choose for the models to run on. */
export type Processor = "auto" | keyof SupportedProviders;

// The order is the reference's - automatic, then fastest to slowest, with the CPU last - and it is
// presentation, which is why it lives in a front end.
/**
 * Every processor this application has a name for, in the order a chooser offers them.
 *
 * **Which** of these a machine is actually offered is not decided here: that is the report's answer,
 * read by the row that draws the choice.
 */
export const PROCESSORS = ["auto", "tensorrt", "cuda", "coreml", "cpu"] as const satisfies readonly Processor[];

const isProcessor = (value: unknown): value is Processor =>
    typeof value === "string" && (PROCESSORS as readonly string[]).includes(value);

// Two surfaces and no third. Both are decorative and neither affects what the application can do,
// which is why it is a pair of names rather than a set of tunable knobs.
/** What the preview canvas draws behind the image: a drifting field of particles, or a grid of dots. */
export type Background = "particles" | "dotted";

// Shaped like `PROCESSORS` because it does the same two jobs: it is the list the control renders
// from, and it is what `isBackground` judges a remembered value against, so the option a user can
// choose and the value the store will accept cannot come apart.
/** Both surfaces, in the order the row offers them - the particle field first, as the design draws it. */
export const BACKGROUNDS = ["particles", "dotted"] as const satisfies readonly Background[];

const isBackground = (value: unknown): value is Background =>
    typeof value === "string" && (BACKGROUNDS as readonly string[]).includes(value);

// **Which formats take a quality, and what each starts at, are Rust's** - `export_formats` publishes both, beside
// the range - so this store names no format and restates no default. What it keeps is only what a user moved: a
// format with no entry is written at its published default, which is also what a first launch and Reset to defaults
// leave every format at. A record written before the defaults moved - every lossy format at its then default - reads
// the same, entry by entry.
/**
 * The quality a user chose for each lossy format, where they chose one. **Per format rather than one shared number**:
 * the scales are not comparable across encoders, so one value would be a setting that meant something different in
 * every row it appeared in.
 */
export type QualityChoices = Partial<Record<ExportFormat, number>>;

// The bound every lossy format publishes today, kept here too because the store clamps a value on the way in - the
// one field handed straight to the native encoders - and it rehydrates synchronously, before any answer from Rust.
export const MIN_QUALITY = 1;
export const MAX_QUALITY = 100;

/** A quality brought inside what the encoders accept, which is what a control is allowed to write. */
export const clampQuality = (value: number) => clampTo(Math.round(value), MIN_QUALITY, MAX_QUALITY);

// Partial rather than defaulted here, which is what keeps the catalogue out of this store: the
// catalogue is an `invoke` and this store rehydrates synchronously at import, so a stored model
// validated here would have to be judged against a promise or against nothing.
/**
 * The user's chosen default model for each enhancement family, as `<codename>_<precision>`.
 *
 * **Partial, and that is the point.** A family the user has never chosen for holds nothing rather
 * than a default written in here. It is resolved where it is drawn instead - see
 * `lib/enhancements.ts` - by one rule that covers absent, valid, and naming a model or precision the
 * library no longer publishes.
 */
export type ModelChoices = Partial<Record<Family, string>>;

/** What the settings surface holds: the preferences themselves, with no actions among them. */
export type SettingsData = {
    language: Language;
    /** Which of the two surfaces the preview canvas draws behind the image. */
    background: Background;
    /**
     * Whether the application may send anonymous usage analytics. Mirrored to Rust by
     * `lib/analytics.ts`, which reads its copy at launch: off stops sending at once, on takes effect
     * at the next launch.
     */
    analytics: boolean;
    processor: Processor;
    models: ModelChoices;
    // The families switched **off** rather than those switched on, which is what makes both defaults
    // hold without a repair step: nothing stored means every family is allowed, and a family a later
    // version adds is absent from the list, so it is allowed too. An inclusion list would need a repair
    // that adds the missing family, and could not tell one the user switched off from one it had never
    // seen.
    /**
     * The enhancements Autopilot may **not** suggest. An analysis hands `ENHANCEMENTS` minus these to the
     * library's family filter, read at the moment it is asked for - see `hooks/useAutopilot.ts`.
     */
    autopilotExcluded: Family[];
    quality: QualityChoices;
};

// Beside the preferences rather than among them: `SettingsData` is what the settings dialog drafts,
// restores and resets, and a flag in there would be drafted by a dialog that never shows it and put
// back to `false` by Reset to defaults - asking a user who has already answered to answer again.
/** What is remembered beside the preferences: facts about the user's history, not choices they edit. */
type SettingsRecord = {
    /** Whether the TensorRT question has been answered, on this launch or any earlier one. */
    tensorrtAsked: boolean;
};

type SettingsStore = SettingsData &
    SettingsRecord & {
        /**
         * Puts a set of preferences in force, and on disk. The settings dialog's Save.
         *
         * The quality record is clamped on the way in.
         */
        apply: (values: Partial<SettingsData>) => void;
        /**
         * Records the answer to the TensorRT question: Yes is the automatic processor, No is CUDA, and
         * either way the question has been answered.
         */
        answerTensorRT: (enable: boolean) => void;
    };

/** The preferences out of a store state, which is what the dialog seeds a draft from and Save hands back. */
export const settingsData = ({
    language,
    background,
    analytics,
    processor,
    models,
    autopilotExcluded,
    quality,
}: SettingsData): SettingsData => ({ language, background, analytics, processor, models, autopilotExcluded, quality });

// A function rather than a constant for two reasons: the language is detected, and a record handed
// to two drafts must not be one object both of them hold.
/**
 * What every preference reads as for a user who has never chosen - read by a first launch and by the
 * settings dialog's Reset to defaults, so the two cannot disagree about what a default is.
 *
 * `models` is empty rather than filled in: an absent family already reads, where it is drawn, as the
 * catalogue's first model at its first precision - see {@link ModelChoices}.
 */
export const settingsDefaults = (): SettingsData => ({
    language: detectLanguage(),
    background: "particles",
    analytics: true,
    processor: "auto",
    models: {},
    autopilotExcluded: [],
    // No choice for any format: each is written at the default Rust publishes for it.
    quality: {},
});

/**
 * A stored quality record, repaired.
 *
 * A format this application has no name for, a zero, a NaN or a value outside the bounds is dropped, which reads as
 * that format's published starting value.
 */
const repairQuality = (value: unknown): QualityChoices => {
    // The stakes are the ones `apply`'s clamp states: the record is handed straight to the native
    // encoders.
    //
    // Out of bounds falls back rather than clamping, which is the one place this differs from the
    // reference. It is what the requirement asks for - a remembered value outside those bounds is
    // replaced with the format's starting value - and it is the more conservative reading: a 4000
    // persisted by something that was not this application is not evidence that the user wanted 100.
    if (value === null || typeof value !== "object") return {};

    return Object.fromEntries(
        Object.entries(value).flatMap(([format, stored]) => {
            const quality = Number(stored);

            return isFormatChoice(format) &&
                format !== "preserve" &&
                Number.isFinite(quality) &&
                quality >= MIN_QUALITY &&
                quality <= MAX_QUALITY
                ? [[format, Math.round(quality)]]
                : [];
        }),
    );
};

/** A stored model record, kept only where it names something - the shape, not the models themselves. */
const repairModels = (value: unknown): ModelChoices => {
    if (value === null || typeof value !== "object") return {};

    return Object.fromEntries(
        Object.entries(value as Record<string, unknown>).filter(([, selection]) => typeof selection === "string"),
    ) as ModelChoices;
};

/**
 * A stored exclusion list, repaired: anything that is not a list reads as none excluded, and an entry
 * naming no enhancement this application presents - a family removed from the library, or junk - is
 * dropped along with any duplicate.
 */
const repairExcluded = (value: unknown): Family[] => {
    if (!Array.isArray(value)) return [];

    return familiesWhere((family) => value.includes(family));
};

// No `immer`: the reference needs it for the Maps in its own store, and two flat records do not.
/**
 * The preferences the settings surface holds, remembered across restarts.
 *
 * Persisted under the Wails application's key, so a user upgrading keeps what they chose. The
 * **values** are this application's - see {@link Processor}.
 *
 * **Nothing reaches here until it is kept.** {@link SettingsStore.apply} - the settings dialog's Save -
 * and {@link SettingsStore.answerTensorRT} are the only things that write this store, so a preference
 * the user has not kept cannot be observed by anything - not by a subscriber, and not by the file.
 *
 * Every field has a stated repair - see `merge` below.
 */
export const useSettingsStore = create<SettingsStore>()(
    persist(
        (set) => ({
            // Only ever used on a first launch: `persist` replaces it with the stored choice on
            // every later boot, which is why there is no "have I detected already?" flag beside the
            // detected language.
            ...settingsDefaults(),
            tensorrtAsked: false,

            /*
             * One action rather than a setter per field: this store holds what is *in force*, and
             * the settings surface has exactly one moment a preference changes, which is Save (the
             * TensorRT question is not the settings surface - see `answerTensorRT`). While the dialog is
             * open the user's edits live in the dialog's own draft - see `features/settings/draft.tsx`
             * - so nothing outside it can observe a preference the user has not kept.
             *
             * Don't hold drafts here instead. Every row would write the store as a control was
             * touched, `persist` would have to be gated against writing them to disk, Cancel would
             * have to restore from a module-global snapshot, and every reader outside the dialog would
             * see drafts unless it read a second store holding the applied copy - a gate that must be
             * remembered per reader. A store that only ever holds committed values has nothing to
             * remember.
             *
             * The quality record is clamped here because it is the one field handed straight to the
             * native encoders, which take a 0 as a genuine request rather than falling back to their
             * own default and write out a garbage image - so the bound belongs on the field, not on
             * the control that happens to be the only writer today.
             */
            apply: (values: Partial<SettingsData>) =>
                set((state) => ({
                    ...state,
                    ...values,
                    ...(values.quality && {
                        quality: Object.fromEntries(
                            Object.entries(values.quality).map(([format, quality]) => [format, clampQuality(quality)]),
                        ),
                    }),
                })),

            // One `set` for both halves, so no state exists - in memory or on disk - in which the
            // processor the answer chose is recorded and the fact that it was answered is not.
            //
            // Yes writes the automatic choice rather than leaving the processor alone: on a first
            // launch the two are the same thing, and writing it states the answer instead of
            // depending on nothing having changed the preference first. No is the next best thing
            // on a machine that offers TensorRT, which is installed as a layer on the CUDA libraries.
            answerTensorRT: (enable: boolean) => set({ processor: enable ? "auto" : "cuda", tensorrtAsked: true }),
        }),
        {
            // The Wails app's key, unchanged.
            name: "settings-storage",

            // Plain `localStorage`, with no gate on it: every write to this store is a Save, so a
            // write reaching disk is exactly the intent.
            storage: createJSONStorage(() => localStorage),

            // Data only. The actions are rebuilt on every launch and a persisted one would be a
            // function that survived the code it was written against.
            partialize: (state: SettingsStore): SettingsData & SettingsRecord => ({
                ...settingsData(state),
                tensorrtAsked: state.tensorrtAsked,
            }),

            /*
             * The repairs, applied as the stored state is merged rather than mutated into place
             * afterwards: `merge` is the one point the persisted value passes through, so a repair
             * here cannot be skipped by a path that writes the store some other way.
             *
             * A repair per field, because a persisted store is the one class of state that can be
             * wrong on a user's machine and right here: a language catalogue is dropped, a graphics
             * driver is removed, a model stops being published.
             *
             * All eight are repaired on rehydrate below, against something already present at that
             * moment - the models in shape only. Whether a stored model is still published, and so
             * which model a family defaults to, is resolved where it is drawn, for the reason
             * `ModelChoices` gives. The processor's second repair is the same
             * kind: whether a *known* provider is one **this machine** offers is the setup report's
             * answer, and the report has not arrived when this rehydrates.
             */
            merge: (persisted, current): SettingsStore => {
                /*
                 * Nothing stored, which is a first launch.
                 *
                 * The guard is load-bearing rather than defensive: `persist` calls `merge` whether
                 * or not storage held anything, so without it the detected language would be
                 * overwritten with English on the one boot that detects - and every user who is not
                 * English would start in English, once, permanently, because that is then what gets
                 * persisted.
                 */
                if (persisted === null || typeof persisted !== "object") return current;

                // `isFirstTensorRT` is the Wails application's name for the same fact, the other way up.
                const stored = persisted as Partial<SettingsData & SettingsRecord> & { isFirstTensorRT?: unknown };

                return {
                    ...current,
                    ...stored,
                    // A catalogue this build no longer ships leaves every `t()` falling back key by
                    // key and the picker on an option that is not in its list.
                    language: isLanguage(stored.language) ? stored.language : DEFAULT_LANGUAGE,
                    // A surface this application has no name for, which for most users is simply
                    // the absence of the field: the `settings-storage` the Wails application wrote
                    // has no background in it at all, because that application drew one surface and
                    // offered no choice. The default is the repair, not a migration.
                    background: isBackground(stored.background) ? stored.background : "particles",
                    analytics: typeof stored.analytics === "boolean" ? stored.analytics : current.analytics,
                    // A provider this application has no name for at all - the Wails app's
                    // `"TensorRT"`, or one removed from the library. Whether a known one is offered
                    // by *this machine* is a separate question, asked where the row is drawn.
                    processor: isProcessor(stored.processor) ? stored.processor : "auto",
                    models: repairModels(stored.models),
                    // Absent for every user who saved before the choice existed, which reads as
                    // every family allowed - the stated default, not a migration.
                    autopilotExcluded: repairExcluded(stored.autopilotExcluded),
                    quality: repairQuality(stored.quality),
                    // Answered here, or answered in the Wails application, which stored `false` once
                    // it had been - so a user upgrading is not asked a second time. Anything else,
                    // including nothing at all, is a question not yet asked.
                    tensorrtAsked: stored.tensorrtAsked === true || stored.isFirstTensorRT === false,
                };
            },
        },
    ),
);
