import { create } from "zustand";
import { cancelSuggest } from "@/ipc/autopilot";
import type { CropInfo } from "@/ipc/crop";
import { report } from "@/lib/report";
import { registerFileOwner } from "@/stores/files";

/** One analysis in flight: the name it was asked under, and the framing it was asked at. */
type Analysis = {
    run: string;
    /** Absent where the photograph was analysed whole. Compared **by reference**, as the crop store hands it. */
    crop?: CropInfo;
};

type AutopilotStore = {
    // **By path, not identity**, because the answer is applied to a stack and stacks are keyed by path: two open
    // files with the same bytes are two stacks, and each gets its own analysis. Slice 1 keyed the backend's table
    // by run name for the same reason.
    //
    // A store rather than component state because the spinner row and the trigger are in different components,
    // and the file-owner registry below is module-level. The reference kept one `analysingPath` in component state
    // and could therefore show only one photograph's spinner; here several can be in flight at once.
    /** The analysis in flight for each photograph, keyed by the file's path. */
    analysing: Map<string, Analysis>;

    // One value is enough: only one photograph is current, and the trigger clears this whenever the current path
    // changes or Autopilot is switched on, so it cannot leak. See design.md D4.
    /**
     * The current photograph, where its analysis just failed - so the trigger does not ask again straight away,
     * fail again and loop. Absent otherwise.
     */
    declined?: string;

    begin: (path: string, run: string, crop: CropInfo | undefined) => void;
    end: (path: string, run: string) => void;
    stop: (path: string) => void;
    stopAll: () => void;
    decline: (path: string) => void;
    clearDeclined: () => void;
};

/** A stop that could not be delivered, which is the bridge going away - the next analysis discovers it anyway. */
const stopFailed = (error: unknown) => report("stopping the Autopilot analysis failed", error);

// Every writer replaces the Map rather than mutating it: see `setTransform` in `stores/transform.ts`.
//
// **Every stop is an action here, never a React cleanup.** Most re-runs of the trigger's effect must not stop
// anything - writing the stack, moving off, this store's own entry appearing - and the reference records what
// tying a stop to a cleanup cost it: the stack write re-ran the effect, the cleanup cancelled the analysis in
// flight, and the spinner stayed up forever. See design.md D2.
//
// Not persisted: an analysis belongs to a photograph that is open.
/**
 * The Autopilot analyses in flight, one per photograph at most, and the one photograph not to ask about again
 * yet.
 *
 * **Not persisted.** A photograph's analysis is stopped when it is closed.
 */
export const useAutopilotStore = create<AutopilotStore>()((set, get) => ({
    analysing: new Map<string, Analysis>(),

    /** Records that one photograph's analysis has been asked for, under `run`, at `crop`. */
    begin: (path: string, run: string, crop: CropInfo | undefined) =>
        set((state) => ({ analysing: new Map(state.analysing).set(path, { run, ...(crop && { crop }) }) })),

    /**
     * Records that one photograph's analysis has settled, **only where the entry still names `run`**.
     *
     * An older analysis settling after a stop and a newer ask - a reframe, or a switch off and on - must not take
     * the newer one's entry, and with it the spinner, down with it.
     */
    end: (path: string, run: string) =>
        set((state) => {
            if (state.analysing.get(path)?.run !== run) return state;

            const analysing = new Map(state.analysing);
            analysing.delete(path);

            return { analysing };
        }),

    /**
     * Stops one photograph's analysis and forgets it. Its answer, if it still arrives, is discarded: the backend
     * answers `stopped`, and the entry it would be checked against is gone.
     *
     * A no-op for a photograph with nothing in flight.
     */
    stop: (path: string) => {
        const analysis = get().analysing.get(path);
        if (!analysis) return;

        void cancelSuggest(analysis.run).catch(stopFailed);

        get().end(path, analysis.run);
    },

    /** Stops every analysis in flight and forgets them all, which is what switching Autopilot off means. */
    stopAll: () => {
        const { analysing } = get();
        if (analysing.size === 0) return;

        for (const { run } of analysing.values()) void cancelSuggest(run).catch(stopFailed);

        set({ analysing: new Map<string, Analysis>() });
    },

    /** Holds the trigger off one photograph until it stops being current or Autopilot is switched on again. */
    decline: (path: string) => set({ declined: path }),

    clearDeclined: () =>
        set((state) => {
            if (state.declined === undefined) return state;

            // The replacing form, because `declined` has to be absent rather than `undefined`: see `retry` in
            // `stores/setup.ts`.
            const { declined: _cleared, ...rest } = state;

            return rest;
        }, true),
}));

/**
 * Whether one photograph is being analysed, which is what the sidebar draws its analysing row on.
 *
 * `undefined` for the path answers false, so a window with nothing open reads as nothing being analysed.
 */
export const useAnalysing = (path: string | undefined) =>
    useAutopilotStore((state) => path !== undefined && state.analysing.has(path));

/*
 * A per-file owner keyed by path. This registration is the whole of "closing a photograph stops its analysis":
 * nothing is added to `closeFile`.
 */
registerFileOwner({
    forget: (path) => useAutopilotStore.getState().stop(path),
    forgetAll: () => useAutopilotStore.getState().stopAll(),
});
