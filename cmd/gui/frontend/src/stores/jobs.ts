import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

/**
 * Tracks how many backend inference calls are currently in flight, so the UI can show that a model is busy — the AI
 * processor can't be switched while one is running, because the change only takes effect by unloading every loaded
 * model.
 *
 * This is UI state, not a safety mechanism: the backend blocks `CleanRegistry` until the inference in flight has
 * finished (see `internal.InferenceMu`), so nothing here can crash the app by getting the count wrong.
 *
 * It's a counter and not a boolean because the preview, an export and face detection can all be running at once. The
 * increments live in `utils/jobs.ts`, applied at the four functions that actually call into Go.
 */
type JobStore = {
    activeJobs: number;

    beginJob: () => void;
    endJob: () => void;
};

export const useJobStore = create(
    immer<JobStore>((set, _) => ({
        activeJobs: 0,

        beginJob: () => {
            set((state) => {
                state.activeJobs += 1;
            });
        },

        endJob: () => {
            set((state) => {
                // Clamp at 0 so an unbalanced call can't leave the counter negative, which would make `selectIsBusy`
                // read false while a job is still running.
                state.activeJobs = Math.max(0, state.activeJobs - 1);
            });
        },
    })),
);

/** True while at least one backend inference call is in flight. */
export const selectIsBusy = (state: JobStore) => state.activeJobs > 0;
