import { invoke } from "@tauri-apps/api/core";

// Written a second time in `crates/gui/src/frontend.rs`, as a `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime: `log.test.ts`
// pins this half, and `generate_handler![]` the Rust one by refusing to compile against a function
// that does not exist.
const LOG_COMMAND = "log";

/**
 * One of the window's failures, as Rust's `WindowRecord` reads it.
 *
 * Rust decides the rest: the record's target, and how long each text may be.
 */
export type WindowRecord = {
    /** `error` for a render crash, which the window does not survive; `warn` for everything else. */
    level: "warn" | "error";
    /** What the window was doing, in its own sentence. */
    message: string;
    /** The error's own text. */
    error?: string;
    stack?: string;
    /** For a render crash, the components it was thrown under. */
    componentStack?: string;
};

/**
 * Write `record` into the log file. Only the file: Rust keeps it from the collector, because
 * `lib/report.ts` sends the same failure to Grafana through Faro.
 *
 * Call it through `report` in `lib/report.ts`, which decides what is worth a record; this is only the
 * wire.
 */
export const log = (record: WindowRecord) => invoke<void>(LOG_COMMAND, { record });
