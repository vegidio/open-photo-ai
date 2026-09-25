import { report } from "./report";

/**
 * Records what escapes everything else: an error nothing caught, and a promise rejection nothing
 * handled. Both at warning severity, through `report`, so a command's tagged rejection nobody handled
 * is still skipped - Rust has it.
 *
 * Installed in `main.tsx` before React mounts, so a failure in the first render's own effects is
 * caught. A render crash inside the error boundary never arrives here: React 19 sends a caught error to
 * `onCaughtError`, not to `window`. One outside it - in the providers, or in `RevealWindow` - does, and
 * this is its one record.
 *
 * Returns the removal of both handlers.
 */
export const watchUnhandled = (): (() => void) => {
    // Without capture, so a resource that failed to load on an element, whose `error` does not bubble,
    // stays out: that is a missing image, not a failure of the window's code.
    const onError = (event: ErrorEvent) => report("an error nothing caught", event.error ?? event.message);
    const onRejection = (event: PromiseRejectionEvent) => report("a promise rejection nothing handled", event.reason);

    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);

    return () => {
        window.removeEventListener("error", onError);
        window.removeEventListener("unhandledrejection", onRejection);
    };
};
