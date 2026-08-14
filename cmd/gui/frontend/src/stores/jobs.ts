import { immer } from 'zustand/middleware/immer';
import { create } from 'zustand/react';

/**
 * Tracks how many backend inference calls are currently in flight.
 *
 * The backend destroys its ONNX sessions the moment `CleanRegistry` is called, without waiting for running inference
 * to finish — doing so while a model is mid-run frees a native session underneath it and takes the whole app down
 * (issue #34). This counter is what lets the UI keep those two apart: the AI processor can't be changed while a job is
 * running, and the registry is only cleaned once everything is idle.
 *
 * It's a counter and not a boolean because the preview, an export and face detection can all be running at once.
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
                // Clamp at 0 so an unbalanced call can't leave the counter negative, which would make `isBusy` read
                // false while a job is still running — the exact state this store exists to prevent.
                state.activeJobs = Math.max(0, state.activeJobs - 1);
            });
        },
    })),
);

/** True while at least one backend inference call is in flight. */
export const isBusy = () => useJobStore.getState().activeJobs > 0;

/**
 * Runs `fn` as soon as no inference is in flight — immediately when already idle, otherwise once the last running job
 * finishes. Used to hold back work that isn't safe to do while a model is running.
 */
export const runWhenIdle = (fn: () => void) => {
    if (!isBusy()) {
        fn();
        return;
    }

    const unsubscribe = useJobStore.subscribe((state) => {
        if (state.activeJobs > 0) return;

        unsubscribe();
        fn();
    });
};
