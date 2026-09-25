import { create } from "zustand";
import { cancelExport } from "@/ipc/export";
import { report } from "@/lib/report";
import { useAutopilotStore } from "@/stores/autopilot";

/** Where a batch is: before Save, while it runs, or once it has finished, failed in part or been aborted. */
export type BatchPhase = "idle" | "running" | "ended";

/** Where one queued file is. */
export type Row =
    | { stage: "queued" }
    /** Being analysed before it is enhanced: by Autopilot, or to find the faces a face recovery needs. */
    | { stage: "analysing" }
    /** Its enhancements are running; `fraction` is how far through the chain, from 0 to 1. */
    | { stage: "enhancing"; fraction: number }
    | { stage: "writing" }
    /** Written: `path` is what was actually written, the numbered name where one was, and `bytes` how much. */
    | { stage: "done"; path: string; bytes: number }
    /** Not exported; `reason` is what its tooltip shows and a click copies, composed when it failed. */
    | { stage: "failed"; reason: string };

/** What an Abort has to stop: the file being worked on, and what is in flight for it. */
export type Current = {
    path: string;
    /** The export's name, once it has been asked for. */
    run?: string;
    /** Whether an Autopilot analysis is in flight for it. A face detection has no stop, and is not marked. */
    analysing?: boolean;
};

type ExportBatchStore = {
    phase: BatchPhase;
    /** The paths queued when the dialog opened, in the order they are exported. Fixed for the dialog's life. */
    queue: string[];
    rows: Map<string, Row>;
    current?: Current;
    /** Whether Abort was pressed during this batch. The loop reads it after every await. */
    aborted: boolean;

    /** Snapshots the queue and marks every row Queued. The dialog calls it as it opens. */
    open: (paths: string[]) => void;
    /** Every row Queued again and nothing written, for Export again. Starts nothing. */
    reset: () => void;
    start: () => void;
    finish: () => void;
    setStage: (path: string, row: Row) => void;
    setCurrent: (current?: Current) => void;
    /** Stops what is in flight and lets no further file start. A no-op unless a batch is running. */
    abort: () => void;
};

const QUEUED: Row = { stage: "queued" };

/** Every path in `queue` Queued. */
const queued = (queue: readonly string[]) => new Map<string, Row>(queue.map((path) => [path, QUEUED]));

/** A stop that could not be delivered, which is the bridge going away. */
const stopFailed = (error: unknown) => report("stopping the export failed", error);

// A store rather than the dialog's state because the title's summary, the overall bar, the rows, the buttons and the
// close policy are several components, and all of them must agree about whether a batch is running - the reason the
// reference moved its `runState` into its store. The loop that drives it is `features/export/batch.ts`.
//
// Every writer replaces the Map rather than mutating it: see `setTransform` in `stores/transform.ts`.
/**
 * The export batch: its phase, its queue, each row's stage and what Abort would stop.
 *
 * **Not persisted.** A batch belongs to the dialog that ran it.
 */
export const useExportBatchStore = create<ExportBatchStore>()((set, get) => ({
    phase: "idle",
    queue: [],
    rows: new Map<string, Row>(),
    aborted: false,

    open: (paths: string[]) => set({ phase: "idle", queue: [...paths], rows: queued(paths), aborted: false }),

    reset: () =>
        set((state) => {
            const { current: _current, ...rest } = state;

            return { ...rest, phase: "idle", rows: queued(state.queue), aborted: false };
        }, true),

    start: () => set({ phase: "running", aborted: false }),

    finish: () =>
        set((state) => {
            const { current: _current, ...rest } = state;

            return { ...rest, phase: "ended" };
        }, true),

    setStage: (path: string, row: Row) => set((state) => ({ rows: new Map(state.rows).set(path, row) })),

    setCurrent: (current?: Current) =>
        set((state) => {
            const { current: _previous, ...rest } = state;

            return current === undefined ? rest : { ...rest, current };
        }, true),

    abort: () => {
        const { phase, current } = get();
        if (phase !== "running") return;

        set({ aborted: true });

        // What is in flight is stopped here and the loop does the rest: it observes `aborted` after the await it is
        // in, puts the row back to Queued and starts nothing more. A face detection has no stop, and is waited for.
        if (current?.run !== undefined) void cancelExport(current.run).catch(stopFailed);
        if (current?.analysing) useAutopilotStore.getState().stop(current.path);
    },
}));

/** What the title bar summarises. */
export type BatchSummary = {
    phase: BatchPhase;
    total: number;
    written: number;
    failed: number;
    /** The rows still Queued, less the one being worked on. */
    remaining: number;
};

/** The counts behind the title's summary, out of a store state. */
export const batchSummary = ({ phase, queue, rows, current }: ExportBatchStore): BatchSummary => {
    let written = 0;
    let failed = 0;
    let remaining = 0;

    for (const path of queue) {
        const stage = rows.get(path)?.stage;

        if (stage === "done") written++;
        else if (stage === "failed") failed++;
        else if (stage === "queued" && path !== current?.path) remaining++;
    }

    return { phase, total: queue.length, written, failed, remaining };
};

/** How far the row being worked on is: its chain's fraction, all of it once writing, none before. */
const rowFraction = (row: Row | undefined) => {
    if (row?.stage === "enhancing") return row.fraction;

    return row?.stage === "writing" ? 1 : 0;
};

/**
 * How far the whole batch is, from 0 to 1: the finished rows, Done or Failed, plus the fraction of the row being
 * worked on, over the number of rows.
 */
export const overallFraction = (state: ExportBatchStore) => {
    const { total, written, failed } = batchSummary(state);
    if (total === 0) return 0;

    const current = state.current && rowFraction(state.rows.get(state.current.path));

    return (written + failed + (current ?? 0)) / total;
};
