import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { type FormatChoice, isFormatChoice } from "@/lib/export";

/** What the export dialog's settings column holds, remembered across launches. The quality is Settings'. */
export type ExportSettingsData = {
    /** Put in front of every file's name. */
    prefix: string;
    /** Put after every file's name, before its extension. */
    suffix: string;
    /** The folder to write into, or absent for the directory each source is in. */
    location?: string;
    /** Whether a file already at a destination may be replaced. Without it, a numbered name is written instead. */
    overwrite: boolean;
    format: FormatChoice;
};

type ExportSettingsStore = ExportSettingsData & {
    setPrefix: (prefix: string) => void;
    setSuffix: (suffix: string) => void;
    /** Chooses a folder, or the directory each source is in where `location` is absent. */
    setLocation: (location?: string) => void;
    setOverwrite: (overwrite: boolean) => void;
    setFormat: (format: FormatChoice) => void;
};

/** What every setting reads as for a user who has never chosen: the reference's own defaults. */
export const exportSettingsDefaults = (): ExportSettingsData => ({
    prefix: "",
    suffix: "",
    overwrite: false,
    format: "png",
});

/** The settings out of a store state, without its actions: what a batch is started with. */
export const exportSettingsData = ({
    prefix,
    suffix,
    location,
    overwrite,
    format,
}: ExportSettingsData): ExportSettingsData => ({
    prefix,
    suffix,
    ...(location !== undefined && { location }),
    overwrite,
    format,
});

/** A stored format, repaired: the reference's `jpg` is this application's `jpeg`, and anything unknown is PNG. */
const repairFormat = (value: unknown): FormatChoice => {
    if (value === "jpg") return "jpeg";

    return isFormatChoice(value) ? value : "png";
};

// Written as the user types rather than drafted and saved, as the reference writes it: these are the choices the
// queue's names are drawn from, and a queue that did not follow them while idle would be showing names it will not
// write. Only the quality is a draft, because it is Settings' value, which the Settings dialog also edits.
/**
 * The export dialog's settings, remembered across launches.
 *
 * Persisted under the Wails application's key, so a user upgrading keeps what they chose; its `jpg` is repaired to
 * this application's `jpeg` on the way in. Every field has a stated repair - see `merge` below.
 */
export const useExportSettingsStore = create<ExportSettingsStore>()(
    persist(
        (set) => ({
            ...exportSettingsDefaults(),

            setPrefix: (prefix: string) => set({ prefix }),
            setSuffix: (suffix: string) => set({ suffix }),
            // The state replaced whole rather than merged: under `exactOptionalPropertyTypes` the original directory
            // is an absent `location`, which merging cannot write.
            setLocation: (location?: string) =>
                set((state) => {
                    const { location: _previous, ...rest } = state;

                    return location === undefined ? rest : { ...rest, location };
                }, true),
            setOverwrite: (overwrite: boolean) => set({ overwrite }),
            setFormat: (format: FormatChoice) => set({ format }),
        }),
        {
            // The Wails app's key, unchanged.
            name: "export-storage",
            storage: createJSONStorage(() => localStorage),

            // Data only, for the reason `stores/settings.ts` gives. The reference's `key` and `runState` are session
            // state; here the batch lives in `stores/exportBatch.ts`, which is not persisted at all.
            partialize: (state: ExportSettingsStore): ExportSettingsData => exportSettingsData(state),

            merge: (persisted, current): ExportSettingsStore => {
                if (persisted === null || typeof persisted !== "object") return current;

                const stored = persisted as Partial<Record<keyof ExportSettingsData, unknown>>;
                const { location: _location, ...rest } = current;

                return {
                    ...rest,
                    prefix: typeof stored.prefix === "string" ? stored.prefix : "",
                    suffix: typeof stored.suffix === "string" ? stored.suffix : "",
                    // Anything but a path is the original directory, which is also what the reference's absent
                    // `location` meant.
                    ...(typeof stored.location === "string" && stored.location !== "" && { location: stored.location }),
                    overwrite: stored.overwrite === true,
                    format: repairFormat(stored.format),
                };
            },
        },
    ),
);
