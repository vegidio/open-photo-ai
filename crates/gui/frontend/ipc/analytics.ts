import { invoke } from "@tauri-apps/api/core";

// Written a second time in `crates/gui/src/analytics.rs`, as a `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime:
// `analytics.test.ts` pins this half, and `generate_handler![]` the Rust one by refusing to compile
// against a function that does not exist.
const SET_ANALYTICS_COMMAND = "set_analytics";

/**
 * Why the choice could not be recorded: Rust's `AnalyticsError`, tagged like every other command's.
 * `message` is the reason in full, in English and untranslated.
 *
 * An opt-out has stopped sending even when this is the answer: only the file the next launch reads
 * could not be written.
 */
export type AnalyticsError = {
    kind: "setAnalytics";
    message: string;
};

/**
 * Mirror the user's Analytics choice to Rust, which decides at launch - before this window exists -
 * whether to send anything.
 *
 * Off stops sending at once. On takes effect at the next launch.
 */
export const setAnalytics = (enabled: boolean) => invoke<void>(SET_ANALYTICS_COMMAND, { enabled });
