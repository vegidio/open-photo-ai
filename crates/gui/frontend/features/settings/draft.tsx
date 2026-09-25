import { createContext, type ReactNode, use, useMemo, useState } from "react";
import { type SettingsData, settingsData, useSettingsStore } from "@/stores/settings";

/** The preferences as the dialog currently has them, and the way a row changes one. */
export type SettingsDraft = {
    /** What every row reads. Seeded from the store when the dialog opens, and discarded if it cancels. */
    values: SettingsData;
    /** Writes one or more preferences into the draft. Never reaches the store, and never reaches disk. */
    update: (patch: Partial<SettingsData>) => void;
};

// Drafts held *in* the store, behind gated writes to disk, keep the file honest but not the store:
// every reader outside the dialog sees drafts, each one then needs a second store holding the applied
// copy, and a reader that does not get one reads drafts silently - the canvas, the inference run and
// the add menu among them. A draft that never leaves the dialog has no such readers to miss.
//
// A context rather than props threaded through four section components: the rows are built from a
// table in `SettingsDialog`, so there is no call site to thread through.
/**
 * Where a settings row reads and writes, which is the dialog rather than the store.
 *
 * **This is what makes a setting a draft.** The preferences the application is actually running on
 * live in `stores/settings.ts` and change only when Save calls `apply`. Everything the user touches
 * before then is here, in state belonging to the dialog - so a preference the user has not kept
 * cannot be observed by anything outside these rows, and Cancel discards it by never applying it: the
 * next open re-seeds the draft from what is in force.
 */
const DraftContext = createContext<SettingsDraft | undefined>(undefined);

export const SettingsDraftProvider = ({ draft, children }: { draft: SettingsDraft; children: ReactNode }) => (
    <DraftContext value={draft}>{children}</DraftContext>
);

/**
 * The draft a row reads and writes.
 *
 * Throws outside the dialog.
 */
export const useSettingsDraft = (): SettingsDraft => {
    const draft = use(DraftContext);

    // Rather than falling back to the store: a row rendered without a draft would edit preferences in
    // force, one control at a time, with nothing to cancel it.
    if (!draft) throw new Error("a settings row was drawn outside the settings dialog's draft");

    return draft;
};

/**
 * The draft the dialog owns: what the rows write, seeded from what is in force and re-seeded by `reopen`.
 *
 * `seed` overrides preferences in the first snapshot only, for a test that wants a row to open on a
 * value the store does not hold; the dialog passes none, and `reopen` re-seeds from the store alone.
 */
export const useDraftState = (seed?: Partial<SettingsData>) => {
    // Seeded through `settingsData` so the copy carries the preferences and none of the store's
    // actions, and re-seeded by `reopen` rather than by an effect - the snapshot has to be taken before
    // anything on the surface is touched, and an effect would also re-run on a remount and quietly
    // re-baseline a dialog the user had already edited.
    const [values, setValues] = useState<SettingsData>(() => ({
        ...settingsData(useSettingsStore.getState()),
        ...seed,
    }));

    return useMemo(
        (): { draft: SettingsDraft; reopen: () => void } => ({
            draft: {
                values,
                update: (patch: Partial<SettingsData>) => setValues((current) => ({ ...current, ...patch })),
            },
            reopen: () => setValues(settingsData(useSettingsStore.getState())),
        }),
        [values],
    );
};
