import { call } from "./invoke";
import { once } from "./once";

// Written a second time in `crates/gui/src/lib.rs`, as the `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime: `app.test.ts`
// pins this half, and `generate_handler![]` the Rust one by refusing to compile against a function
// that does not exist.
const VERSION_COMMAND = "version";

/**
 * The application version, as `opai::version()` reports it - the same string `cli` prints. Nothing is
 * computed on the way through, so what lands here is what the core library said.
 */
export const appVersion = () => call<string>(VERSION_COMMAND);

// Written a second time in `crates/gui/src/update.rs`, as the `#[tauri::command]` function's name;
// `app.test.ts` pins this half.
const IS_OUTDATED_COMMAND = "is_outdated";

// Kept because each ask is a request against GitHub's sixty an hour, and the answer is the launch's:
// a remount (React's strict mode in development) must not spend a second one. The command never
// rejects for a failed check - it answers `false` and logs why - so what is kept is always an answer.
const outdated = once(() => call<boolean>(IS_OUTDATED_COMMAND));

/**
 * Whether a newer release than the running build is published. `false` also when the check could
 * not answer, which Rust has already logged.
 *
 * **Asked once per launch and kept**, as {@link once} keeps it.
 */
export const isOutdated = () => outdated.get();

/** Forgets the kept answer. For tests alone - see {@link once}. */
export const forgetOutdated = () => outdated.forget();
