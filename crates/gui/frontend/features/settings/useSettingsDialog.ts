import { useState } from "react";
import i18n from "@/i18n";
import type { SettingKey } from "@/lib/events";
import { track } from "@/lib/faro";
import { type SettingsData, settingsData, useSettingsStore } from "@/stores/settings";
import { useDraftState } from "./draft";

/**
 * `value` as text that two equal preferences always share: a record's keys, and a list's entries, in
 * sorted order. A list here is a set - the families Autopilot leaves out - whichever order the toggles
 * were pressed in.
 */
const canonical = (value: unknown) =>
    JSON.stringify(value, (_key, part: unknown) => {
        if (Array.isArray(part)) return [...part].sort();
        if (typeof part === "object" && part !== null) return Object.fromEntries(Object.entries(part).sort());

        return part;
    });

/**
 * The preferences that differ between `before` and `after`, sorted. Never `analytics`: an opt-out that
 * is reported is not one.
 */
export const changedSettings = (before: SettingsData, after: SettingsData): SettingKey[] =>
    (Object.keys(after) as (keyof SettingsData)[])
        .filter((key): key is SettingKey => key !== "analytics")
        .filter((key) => canonical(before[key]) !== canonical(after[key]))
        .sort();

/**
 * The settings dialog's open state, and the draft that lives exactly as long as it.
 *
 * **One seam for the whole lifecycle.** Opening re-seeds the draft from what is in force; Save puts
 * the draft in force, tells i18next about the language and sends `settings_saved` for what changed;
 * Cancel does nothing at all, because a draft that is never applied needs nothing undone.
 */
export const useSettingsDialog = () => {
    // The draft is React state rather than drafts held in the settings store behind gated writes to
    // disk. That arrangement makes the lifecycle three steps that have to be paired correctly - a
    // begin on the way in, and either a commit or a cancel on the way out - and neither a missed begin
    // (every preference written the instant a control is touched, with Cancel unable to take it back)
    // nor a missed commit (settings silently stop persisting for the rest of the session) throws, so
    // both fail quietly. Here there is no pairing to get wrong: Cancel is the absence of a write.
    const [open, setOpen] = useState(false);
    const { draft, reopen } = useDraftState();
    const apply = useSettingsStore((state) => state.apply);

    const close = (save: boolean) => {
        if (save) {
            // Read around the write rather than off the draft: the store is what is in force, and it
            // clamps what the draft held.
            const before = settingsData(useSettingsStore.getState());
            apply(draft.values);
            const changed = changedSettings(before, settingsData(useSettingsStore.getState()));

            // Which settings people touch. Their new values are in the next launch's `app_ready`.
            if (changed.length > 0) track("settings_saved", { changed });

            // Handed to i18next here rather than by the store, because the language is the one
            // preference whose effect lives outside this application's own state: i18next holds what
            // is in force, exactly as the store does for everything else.
            void i18n.changeLanguage(draft.values.language);
        }

        setOpen(false);
    };

    return {
        open,
        draft,

        /** Opens the dialog on a draft of what is currently in force. */
        openSettings: () => {
            reopen();
            setOpen(true);
        },

        /** Ends the dialog, keeping the draft (`close(true)`, Save) or discarding it (`close(false)`). */
        close,
    };
};
