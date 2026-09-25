import { platform } from "@tauri-apps/plugin-os";

// One of the two questions the frontend asks about the host, and this file is the only place
// `@tauri-apps/plugin-os` is touched. It is a function rather than a module-scope constant so that
// jsdom - where the global the plugin reads does not exist - fails inside a call a test has mocked,
// rather than while importing whatever component happened to pull this module in.
//
// `platform()` is synchronous, and that is the reason the plugin is here at all rather than a
// `#[tauri::command]` beside `version`. The plugin's `init()` installs a `js_init_script` built from
// `std::env::consts`, so `window.__TAURI_OS_PLUGIN_INTERNALS__` is populated before any of this
// application's code runs and the answer is a property read. The navbar's 86px traffic-light inset
// depends on it: awaited, the navbar would paint flush and then jump on the next tick.
/** Whether the application is running on macOS. */
export const isMacOs = () => platform() === "macos";

// Asked for one reason: Windows is the only platform that reports a file drop's position in device
// pixels. `useDroppedImages` needs it to hit-test the drop against the canvas - see that hook for
// where the three platforms differ and why. Synchronous for the same reason `isMacOs` is, and through
// the same plugin.
/** Whether the application is running on Windows. */
export const isWindows = () => platform() === "windows";
