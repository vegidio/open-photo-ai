import { listen } from "@tauri-apps/api/event";
import type { Family, Precision } from "./catalogue";
import type { CropInfo } from "./crop";
import type { Face } from "./faces";
import { call } from "./invoke";
import type { SupportedProviders } from "./setup";

/** Rust command names. Must match `#[tauri::command]` functions in `crates/gui/src/enhance/mod.rs`; renames aren't type-checked across the IPC boundary. */
const ENHANCE_COMMAND = "enhance";
const CANCEL_ENHANCE_COMMAND = "cancel_enhance";
const RELEASE_ENHANCED_COMMAND = "release_enhanced";
const RELEASE_ALL_ENHANCED_COMMAND = "release_all_enhanced";

/** Event name for run progress, matching `PROGRESS_EVENT` in `crates/gui/src/enhance/progress.rs`. Global (not per-call) since reports carry their own run id, letting listeners filter stale runs. */
const PROGRESS_EVENT = "enhance:progress";

/** Processor selector, derived from `SupportedProviders` to stay in sync with backend-added providers. Uses wire spellings (e.g. `tensorrt`), translated to library enum spellings in `enhance/run.rs`. */
export type Processor = "auto" | keyof SupportedProviders;

/**
 * One operation in a chain: a catalogue row, precision, and parameter values.
 *
 * Spelled in the catalogue's vocabulary (`family`, `codename`, `precision`, parameter names) so Rust can match
 * directly without a lookup table. Not a composed string like `up_kyoto_2x_fp32` — avoids a triple contract.
 * `detection` is excluded; faces are fetched via a separate command.
 */
export type Operation = {
    family: Exclude<Family, "detection">;
    /** Model name, per `VariantEntry.codename`. */
    codename: string;
    /** Build variant, per `VariantEntry.precisions`. */
    precision: Precision;
    /**
     * Parameter values by catalogue name (`scale`, `strength`, etc.), in library units.
     * Starts at catalogue defaults; out-of-range values are clamped rather than rejected, since a bad value
     * can only come from a fault, not user input.
     */
    parameters: Record<string, number>;
    /**
     * Face-recovery exceptions to restore, if any. Not the face list itself — just the user's picks,
     * merged in by `withChoice` in `lib/faces.ts` before calling {@link enhance}.
     */
    faces?: FaceChoice;
};

/** User overrides to the default face-recovery selection, keyed by Rust's per-face id. Mirrors Rust's `FaceChoice` in `crates/gui/src/faces.rs`. */
export type FaceChoice = { skipped: string[]; restored: string[] };

/** Current run phase, per Rust's `Stage`. */
type Stage = "installing" | "running";

/**
 * One progress report for a run.
 *
 * `stage` is absent when the operation's result was cached (no work done, no progress jump).
 * `installFraction` appears only during an active model download, since `chainFraction` alone barely
 * moves during a large fetch. Reports are pre-throttled by Rust to ~100 per run.
 */
export type RunProgress = {
    /** Run this report belongs to; discard if it's not the run you're tracking. */
    run: string;
    /** Human-readable operation name (English, untranslated diagnostic), e.g. "Kyoto 4x (FP16)". */
    operation: string;
    /** Enhancement family this operation belongs to — used for UI labeling since `operation` can't be localized. */
    family: Family;
    stage?: Stage;
    /** `0..1`, monotonic, reaches 1 exactly once. */
    chainFraction: number;
    /** `0..1` progress of an active model download, if any. */
    installFraction?: number;
};

/**
 * Outcome of a run: either enhanced or stopped (never a rejection for a user-initiated stop).
 * Carries a result *description*, not pixels — those are fetched separately via `identity`.
 */
export type Enhancement =
    | {
          outcome: "enhanced";
          /** Address for fetching result pixels, derived from source identity + operations applied. */
          identity: string;
          width: number;
          height: number;
          /** Faces found, for chains with face recovery. Absent if none were sought or detection failed. */
          faces?: Face[];
          /** Detection failure message (untranslated); recovery then restored nothing but the chain continued. */
          facesError?: string;
      }
    /** Stopped via {@link cancelEnhance} or superseded by a later run — including one that finished but was discarded. */
    | { outcome: "stopped" };

/** Reason an operation couldn't run, per Rust's `UnknownOperation`. */
type UnknownOperation =
    | { kind: "model"; family: string; codename: string }
    | { kind: "precision"; codename: string; precision: Precision }
    | { kind: "parameter"; name: string };

/**
 * Reason the `enhance` command rejected (excludes user-initiated stops — see {@link Enhancement}).
 * `message` fields are untranslated English diagnostics, meant for bug reports.
 */
export type EnhanceError =
    | { kind: "notReady" }
    | { kind: "unknownSource"; identity: string }
    | { kind: "unknownOperation"; index: number; reason: UnknownOperation }
    | { kind: "unreadableSource"; identity: string; message: string }
    | { kind: "enhance"; message: string };

/**
 * Unique run name generator. Session-scoped prefix + counter avoids collisions across reloads, since the
 * Rust-side run slot outlives the webview. Not `crypto.randomUUID()` — no need for cryptographic randomness.
 */
const SESSION = Math.random().toString(36).slice(2, 10);
let minted = 0;

/** Mints a unique run name for {@link onEnhanceProgress}. Exported so `ipc/faces.ts` shares the same counter/namespace as enhancement runs. */
export const mintRun = () => `${SESSION}-${++minted}`;

/**
 * Runs a chain of enhancements over an open image.
 *
 * Returns the run name immediately (not just a promise), since a stop can race the run and needs the name
 * before the promise settles:
 * ```ts
 * useEffect(() => {
 *     const { run, done } = enhance(identity, operations, processor);
 *     done.then(draw);
 *     return () => cancelEnhance(run);
 * }, [identity, operations, processor]);
 * ```
 *
 * Starting a run always stops any run in progress (enforced backend-side). `source` must be a previously
 * opened image's identity. `crop`, if given, runs the chain over the framed pixels only; omitted means the
 * whole photograph. An empty `operations` chain is valid and resolves with the source's own identity.
 *
 * Rejects with a serialized {@link EnhanceError} (typed `unknown`, as `invoke` rejections carry no type).
 */
export const enhance = (source: string, operations: Operation[], processor: Processor, crop?: CropInfo) => {
    // Minted before the invoke so a cleanup racing the dispatch still has a name to cancel.
    const run = mintRun();

    return { run, done: call<Enhancement>(ENHANCE_COMMAND, { run, source, operations, processor, crop }) };
};

/**
 * Stops a run by the name {@link enhance} returned.
 * No-op if that run isn't the one currently in flight (e.g. already superseded) — protects against racing
 * a cleanup against a newer request. Also works if called before its own run starts. Never rejects.
 */
export const cancelEnhance = (run: string) => call<void>(CANCEL_ENHANCE_COMMAND, { run });

/**
 * Subscribes to progress reports for all runs. One global listener rather than per-run, since each report
 * carries its own {@link RunProgress.run} id for filtering. Returns the unsubscribe function.
 */
export const onEnhanceProgress = (handler: (report: RunProgress) => void) =>
    listen<RunProgress>(PROGRESS_EVENT, (event) => handler(event.payload));

/**
 * Releases the enhanced result for a closed image, identified by the image's identity (not the result's).
 * This is what frees a large enhanced result from memory once its photo is closed. No-op if nothing is
 * held for that identity; an in-flight run is left to {@link cancelEnhance}. Never rejects meaningfully.
 */
export const releaseEnhanced = (identity: string) => call<void>(RELEASE_ENHANCED_COMMAND, { identity });

/** Releases all enhanced results, e.g. when every image is closed. Same semantics as {@link releaseEnhanced}, for all at once. */
export const releaseAllEnhanced = () => call<void>(RELEASE_ALL_ENHANCED_COMMAND);
