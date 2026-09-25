import { call } from "./invoke";

// Written a second time in `crates/gui/src/window.rs`, as the `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime:
// `window.test.ts` pins this half, and `generate_handler![]` the Rust one by refusing to compile
// against a function that does not exist.
const READY_COMMAND = "window_ready";

/**
 * Tell Rust the window has something in it, so it can be shown.
 *
 * The window ships hidden - `visible: false` in `tauri.conf.json` - and when this is never called it
 * is shown anyway after a five-second grace period. See `crates/gui/src/window.rs` for why it ships
 * hidden, and for why what is reported is a commit rather than a paint.
 *
 * Nothing is worth awaiting beyond reporting a rejection: a window that will not show is not
 * something the frontend can do anything about.
 */
export const windowReady = () => call<void>(READY_COMMAND);
