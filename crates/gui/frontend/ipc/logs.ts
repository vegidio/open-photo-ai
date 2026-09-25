import { call } from "./invoke";

// Written a second time in `crates/gui/src/logs.rs`, as a `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime: `logs.test.ts`
// pins this half, and `generate_handler![]` the Rust one by refusing to compile against a function
// that does not exist.
const REVEAL_LOG_COMMAND = "reveal_log";

/**
 * Why one of the log commands rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `LogsError`. Both arms reach this one command:
 * `revealLog` resolves the log path before it opens anything, so `logPath` is how it reports that the
 * location could not be determined at all, as distinct from a file manager that would not open.
 * `message` is the reason in full, in English and untranslated, for the same reason `SetupError`'s is:
 * it is a diagnostic to be copied into a bug report.
 */
export type LogsError = {
    kind: "logPath" | "revealLog";
    message: string;
};

// A command of this application's own rather than `@tauri-apps/plugin-opener` directly. The plugin
// still does the revealing - it holds the three platform implementations - but its own command is
// `async`, so Tauri runs it on a runtime worker, and all three of its implementations break there:
// macOS aborts the process, Linux panics and never settles the promise, and Windows COM wants a
// main-thread apartment. `crates/gui/src/reveal.rs` has the references. The Rust side therefore
// resolves the path and picks the thread.
/**
 * Show this application's log file to the user in the platform's own file manager, selected.
 *
 * **Takes no path**: Rust resolves it, and nothing this file sends decides which file is opened.
 *
 * **The rejection is propagated rather than swallowed.** A caller that cannot open the file manager
 * has to say so: the user is already diagnosing something else, and a button that quietly does
 * nothing leaves them with no file and no reason.
 */
export const revealLog = () => call<void>(REVEAL_LOG_COMMAND);
