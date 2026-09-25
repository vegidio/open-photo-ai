import { create } from "zustand";
import type { Failure, PlanRow, ProgressRow, RowState, SetupEvent, SupportedProviders } from "@/ipc/setup";

/** What a row of the setup dialog says, including the one state no component reports. */
export type SetupRowState = RowState | "queued";

/** One component the dialog draws: its name, its published size and where it has got to. */
export type SetupRow = {
    name: string;
    size: number;
    state: SetupRowState;
    /** `0..1`. Zero for a row that has reported nothing. */
    fraction: number;
};

/** Where the application's start-up has got to. */
export type SetupStatus = "idle" | "running" | "ready" | "failed";

/** Why the last attempt did not finish: the reason to show, and the shape to show it in. */
export type SetupFailure = {
    kind: Failure;
    message: string;
};

type SetupStore = {
    /** The plan's rows, in the order they will be installed. Empty until the plan arrives. */
    rows: SetupRow[];
    status: SetupStatus;
    /** Set while the status is `failed`, and only then. */
    failure?: SetupFailure;
    // The settings dialog's processor list is built from it, and is the reason it is kept here
    // rather than read and dropped - the report arrives at the one moment start-up finishes, and is
    // wanted whenever a user opens a dialog that did not exist then.
    /**
     * What this machine can be asked to run on, as the resolved `initialize` reported it.
     *
     * Absent until a setup succeeds, which is the honest reading: before then nothing has decided
     * the answer. Nothing clears it afterwards - not `failed`, not `retry` - and a retry's own
     * success overwrites it.
     *
     * Recorded the way the plan is: taken as Rust gave it, with nothing normalised on the way in.
     */
    providers?: SupportedProviders;

    /** Applies one report from the `initialize` command's channel. */
    apply: (event: SetupEvent) => void;
    /**
     * Records that the command resolved: the application is ready, the dialog goes away and the
     * provider report it answered with is kept.
     */
    succeeded: (providers: SupportedProviders) => void;
    /** Records that the command rejected, keeping the reason for the failure dialog to show. */
    failed: (error: unknown) => void;
    /**
     * Puts the setup dialog back for a fresh attempt. A no-op unless the status is `failed`.
     *
     * Both halves in one set, so the failure cannot outlive the dialog that draws it: a `running`
     * status with a failure still beside it is a state nothing should be able to observe. The rows
     * are left as the previous attempt found them until the retry's own plan replaces them.
     */
    retry: () => void;
};

/**
 * `Σ(sizeᵢ × fractionᵢ) / Σ(sizeᵢ)` over every row of the plan.
 *
 * It cannot exceed 1 or walk backwards unless one of its inputs does. Zero rows answers 0 rather than
 * dividing by nothing.
 */
export const overallFraction = (rows: SetupRow[]) => {
    // Size-weighted rather than a count, because the components differ in size by more than an order
    // of magnitude: a bar driven by the count would sit still through a 612 MB download and then jump
    // a third, which is the dishonest bar the core library's own phase split exists to avoid, rebuilt
    // one layer up.
    //
    // Neither input can move the wrong way: the sizes are fixed by the plan before anything moves,
    // and a component's fraction is documented to be monotonic and to reach exactly 1 only when it is
    // finished.
    const total = rows.reduce((sum, row) => sum + row.size, 0);
    // The state before the plan arrives, which is not on screen, because the dialog does not open
    // until a component reports work.
    if (total === 0) return 0;

    return rows.reduce((sum, row) => sum + row.size * row.fraction, 0) / total;
};

/** How many components are finished, out of every component this machine needs. */
export const installedCount = (rows: SetupRow[]) => rows.filter(isFinished).length;

// Beside `stoppedRow` rather than in the dialog that draws the tracks, because the two ask the same
// question of one state table and must not be able to answer it differently: which rows get a track
// of their own, and which row was the one that stopped. Rust's `RowState::of` has a catch-all arm
// precisely because `Phase` gains variants, so a fourth working state is expected - and when it
// lands, this is the one place it has to be named.
/** Whether this state has work in flight. */
export const isWorking = (state: SetupRowState) => state === "downloading" || state === "extracting";

// The fraction rather than the state, because **a component that installs never reports `installed`**:
// that state is `Phase::AlreadyInstalled`, which the core library sends only for a dependency that was
// on disk before this launch started - exactly one report, terminal, no phases. A component that
// actually downloads ends on `Phase::Extracting` at a fraction of exactly 1, because `Reporter::finish`
// lands the bar on 1 and re-emits whatever phase it was in. So the working states are where finished
// components come to rest, and only the fraction tells them apart from a component still moving.
/** Whether this row finished, whatever it was doing when it did. */
const isFinished = (row: SetupRow) => row.fraction >= 1;

// A component that finished comes to rest in a working phase - see `isFinished` - so the phase alone
// leaves a finished row labelled `Extracting` for the whole of the rest of the launch.
/**
 * What a component **ended up in**, which is not always the state it last reported.
 *
 * This is the settled reading of a row, and it is what a state that has stopped moving is drawn
 * from: the failure dialog's outcomes, and `stoppedRow`'s answer about which component the failure
 * belongs to.
 *
 * The live dialog does not use it directly - see `liveState`, which is this with the hand-off between
 * two components held together.
 */
export const settledState = (row: SetupRow): SetupRowState => (isFinished(row) ? "installed" : row.state);

/**
 * Whether this row is the one the installer has finished with but not yet moved on from.
 *
 * True only in the gap between a component's last report and the next component's first: the row has
 * finished, something follows it, and nothing that follows it has reported anything.
 */
const handingOver = (rows: SetupRow[], row: SetupRow) => {
    // The core library does real work in that gap - the next component's record and fingerprint are
    // checked and its connection opened before a byte arrives to report - so it is a window with no
    // reports in it at all, not an instant.
    const after = rows.slice(rows.indexOf(row) + 1);

    return after.length > 0 && after.every((next) => next.state === "queued");
};

// Without the hold the two changes are two moments with a gap between them, and the gap reads as a
// stall: the finished row goes quiet and the next sits at `Queued` while, as far as the dialog shows,
// nothing at all is happening. Holding it is a deliberate, bounded staleness - the row says it is
// still finishing for as long as the library is between components, which is the closest the wire has
// to a report that it is. With the hold, the hand-off is drawn as it actually happens - one component
// at a time, each awaited before the next is started.
//
// The failure dialog reads settled because a failure raised in that gap belongs to the component
// being started, or to no component at all; a held row would put it on the one that had just
// succeeded, which is the whole class of mistake `stoppedRow` exists not to make.
//
// The last row is not held because the dialog stays up while the runtime is loaded, and a final row
// still claiming to expand through that would be the same stall with nothing after it to explain the
// wait.
/**
 * The state a row is drawn in **while the install is running**.
 *
 * `settledState`, except that a component which finished keeps its working label until its successor
 * starts. Both rows then change in the same report: the one above becomes `Installed` and the one
 * below becomes `Downloading` together.
 *
 * It is **not** what the failure dialog reads: a state that has stopped moving is read settled.
 *
 * The last row has nothing to hand over to, so it becomes `Installed` as soon as it finishes.
 */
export const liveState = (rows: SetupRow[], row: SetupRow): SetupRowState =>
    isFinished(row) && handingOver(rows, row) ? row.state : settledState(row);

// Derived rather than written into the rows, and derivable because the core library installs strictly
// sequentially: each dependency is awaited before the next is started, so there is never more than
// one in flight. Every row before the failing one has reported a terminal fraction and every row
// after it has reported nothing. A derived answer cannot disagree with the rows it is derived from,
// which a `stopped: string` written beside them could.
//
// The **settled** state rather than the reported one, which is what makes this the row the dialog marks
// rather than the first row that happens to still carry a working phase. Every component that finished
// by installing carries one of those - see `settledState` - so reading the report would blame the first
// component that succeeded and draw the one that actually stopped as never reached. Settled rather
// than live for the same reason: a failure that arrives during a hand-off must not land on the
// component that had just finished.
/**
 * The component that was working when the failure arrived, where there was one.
 *
 * `undefined` is a real answer rather than a defensive one: a failure raised before anything was
 * probed, and a failure after every component was installed, both belong to no component. On a
 * machine where everything is already on disk and only the runtime will not start, that is the
 * common case.
 */
export const stoppedRow = (rows: SetupRow[]) => rows.find((row) => isWorking(settledState(row)));

// Four shapes rather than one string with holes in it, because what there is to say changes with the
// plan rather than only the numbers in it: a machine with one component has no "of" to state, a
// failure on the first component has no count worth stating, and a failure with components behind it
// has to account for them or leave a user wondering what the untouched rows mean.
//
// Here rather than in the dialog because it is a reading of the same rows `stoppedRow` reads, and the
// two must not be able to disagree about which attempt this was: a header saying nothing was reached
// over a list marking a component as the one that stopped is the contradiction this shares a module to
// avoid.
/**
 * Which sentence the failure dialog's header adds about how far the attempt got.
 *
 * `undefined` where there is nothing to add - a failure belonging to no component, or one raised
 * before a plan arrived - and the header then carries its first sentence alone. The two lines it
 * reserves are what stops that being a shorter dialog.
 */
export type StoppedShape = "only" | "first" | "last" | "rest";

export const stoppedShape = (rows: SetupRow[]): StoppedShape | undefined => {
    const stopped = stoppedRow(rows);
    if (!stopped) return undefined;

    // The single-row plan, which is every macOS launch: "the first one" implies others, and there are
    // none.
    if (rows.length === 1) return "only";

    // Nothing to count yet. "0 of 4 finished" is a figure the bar above already carries, and stating
    // it here would be the count twice for the one shape where it says nothing.
    if (!rows.some(isFinished)) return "first";

    // Whether anything is behind the failure decides whether the untouched rows need accounting for.
    return rows.some((row) => row.state === "queued") ? "rest" : "last";
};

/** A rejection from `initialize`, read as the failure it is. Anything unrecognised answers `other`. */
const setupFailure = (error: unknown): SetupFailure => {
    // The value an `invoke` rejects with is `unknown`: Tauri deserializes whatever crossed the
    // boundary and the promise carries no type for it. In practice it is the serialized `SetupError`,
    // and this is the one place that is checked rather than assumed - a rejection that is not one is
    // still a launch that failed and still has to be drawn.
    //
    // `other` is the same safe direction Rust's `_` arm takes: no advice that might be false, and the
    // button kept.
    if (typeof error === "object" && error !== null && "message" in error && "failure" in error) {
        const { failure, message } = error as { failure: unknown; message: unknown };

        if (typeof message === "string" && (failure === "transfer" || failure === "unrecoverable")) {
            return { kind: failure, message };
        }

        if (typeof message === "string") return { kind: "other", message };
    }

    return { kind: "other", message: String(error) };
};

// A store rather than `useState` in `App.tsx` because the reports arrive from outside React and two
// components read the result: `App` decides whether the dialog exists, the dialog draws the rows.
//
// Not persisted because a plan describes this launch on this machine, and a stale one restored from
// `localStorage` would be a dialog describing the previous run.
/**
 * The plan and each component's progress, as they arrive from Rust.
 *
 * **Not persisted.**
 */
export const useSetupStore = create<SetupStore>()((set) => ({
    rows: [],
    status: "idle",

    apply: (event: SetupEvent) =>
        set((state) => {
            if (event.kind === "plan") {
                // The plan replaces the rows rather than merging into them. It arrives once per
                // initialization, before anything moves, and a retry's plan is the authority on what
                // that attempt will do.
                return { rows: event.rows.map(queued) };
            }

            return {
                rows: state.rows.map((row) => (row.name === event.name ? advanced(row, event) : row)),
                // `running` on the first report of *work*, not on the plan: every component ahead of
                // this one in the order has already said whether it had anything to do, so no row a
                // user sees is drawn as waiting when it is in fact already installed. A launch whose
                // every report is `installed` never reaches this, and shows no dialog at all.
                //
                // Only ever forward from `idle`: a late report arriving after the command resolved
                // must not reopen a dialog the application has already left behind.
                status: state.status === "idle" && event.state !== "installed" ? "running" : state.status,
            };
        }),

    /*
     * The report is set beside the status rather than in a second call, because the two are one fact:
     * the promise that says start-up finished is the promise that carries what it found. Nothing
     * clears it afterwards, since a later attempt that fails does not un-discover what this machine
     * offers.
     */
    succeeded: (providers: SupportedProviders) => set({ status: "ready", providers }),

    failed: (error: unknown) => set({ status: "failed", failure: setupFailure(error) }),

    retry: () =>
        set((state) => {
            /*
             * Only from `failed`, and the guard is load-bearing rather than defensive: `App` shares
             * one `start` between its mount effect and the failure dialog's Try again, so this runs
             * on every launch. Moving to `running` unconditionally would open the setup dialog
             * before any component had said whether it had work - including on the launches with
             * nothing to install, which are required to show no dialog at all.
             */
            if (state.status !== "failed") return state;

            /*
             * The rows are deliberately not reset. The retry's own plan event replaces them
             * wholesale - that is already what `apply` does with a `plan` - and it arrives within
             * milliseconds of the command starting. Until it does, the dialog shows the previous
             * attempt's rows, which is what the previous attempt actually found; clearing them first
             * would be a frame of the dialog claiming the components that installed did not.
             *
             * The replacing form of `set`, because `failure` has to be *absent* rather than set to
             * `undefined`: `exactOptionalPropertyTypes` is on, so an optional property means the key
             * is not there, and zustand's merging `set` can only add keys. Dropping it in the
             * destructure is what removes it.
             */
            const { failure: _cleared, ...rest } = state;

            return { ...rest, status: "running" };
        }, true),
}));

/** A plan row before it has reported anything. */
const queued = (row: PlanRow): SetupRow => ({ name: row.name, size: row.size, state: "queued", fraction: 0 });

// The plan is where a size is published, and the report carries none. The fraction is already
// monotonic per component, so clamping it here would be a second opinion about a number the library
// guarantees.
/**
 * One row updated by one report: the size is kept from the plan, and the state and fraction are taken
 * as given.
 */
const advanced = (row: SetupRow, report: ProgressRow): SetupRow => ({
    ...row,
    state: report.state,
    fraction: report.fraction,
});
