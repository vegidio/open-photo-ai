import { type ReactNode, StrictMode, useEffect } from "react";
import ReactDOM from "react-dom/client";
// Vendored, not linked. The design file loads Geist from fonts.googleapis.com; `tauri.conf.json`
// pins `font-src 'self' data:` and `style-src 'self' 'unsafe-inline'` with no remote origin in
// either policy, so that link is blocked at runtime with nothing to show for it at build time.
// Self-hosting is the only form that works here, and the right one for a desktop app that must
// render identically offline.
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import { ErrorBoundary } from "@/components/ErrorBoundary";
// Side-effect import: runs i18next's init before anything renders. ESM evaluates every import
// before this module's body, so the catalogues are always loaded by the time createRoot().render()
// runs. The rule it imposes on the rest of the app is that `t()` is never called at module scope -
// that would capture the language at evaluation time and never update when it changes.
import "@/i18n";
import { appVersion, telemetryIds } from "@/ipc/app";
import { windowReady } from "@/ipc/window";
import { mirrorAnalytics } from "@/lib/analytics";
import { startFaro } from "@/lib/faro";
import { report } from "@/lib/report";
import { watchUnhandled } from "@/lib/unhandled";
import { useSettingsStore } from "@/stores/settings";
import App from "./App";
import { AppProviders } from "./providers";
import "./style.css";

// Not an `as HTMLElement` cast: if the mount point is missing, the window comes up blank and a cast
// would leave nothing to say why.
const container = document.getElementById("root");

if (!container) {
    throw new Error("#root is missing from index.html; there is nothing to mount into.");
}

// First, and not awaited, on the stored Analytics choice: the store has already rehydrated at import.
// What the window reports before Faro starts is dropped rather than queued - it still reaches the file.
startFaro(useSettingsStore.getState().analytics, appVersion, telemetryIds).catch((error: unknown) => {
    report("Failed to start sending the window's telemetry", error);
});

// Before the app mounts, and outside React: the store has already rehydrated at import, and the choice
// is Rust's to act on whether or not anything has rendered. Never unsubscribed - it lasts as long as the
// window does.
mirrorAnalytics();

// Before the app mounts too, so a failure in the first render's own effects is recorded. Never removed,
// for the same reason.
watchUnhandled();

/**
 * Shows the window, once React has a tree in the DOM.
 *
 * A rejection is reported and dropped.
 */
const RevealWindow = (): ReactNode => {
    useEffect(() => {
        // The window ships hidden (`visible: false` in `tauri.conf.json`) because everything before this
        // point takes time a user would otherwise spend watching an empty webview: the bundle parses,
        // i18next's catalogues load, and React builds its first tree. On a release build that is a couple
        // of hundred milliseconds; under `pnpm dev`, where Vite serves every module unbundled, it is
        // seconds. Hidden, none of it is on screen. See `crates/gui/src/window.rs` for the other half,
        // including the grace period that shows the window if this never runs.
        //
        // **A mount effect rather than `requestAnimationFrame`, and that is the whole design.** The
        // obvious way to wait for a first paint is a pair of animation frames, and it does not work here:
        // a window that is ordered out does not render, so the webview produces no frames and the
        // callback never fires. Measured, not reasoned about: it costs the full five-second grace period
        // on every launch. Anything else that waits on rendering has the same defect,
        // `document.fonts.ready` included: WebKit has no text on screen to need a font for, so the
        // promise stays pending. Waiting to be shown is not something a hidden window can do.
        //
        // What is left is React's own commit, which needs no frames: passive effects are flushed through
        // React's scheduler, so this runs whether or not anything has been painted. By the time it does,
        // the DOM is complete, so the window's first painted frame is the finished shell rather than a
        // step toward it.
        //
        // StrictMode double-invokes this in development, so the call is made twice. Rust answers the
        // second one by doing nothing: see `reveal` in `crates/gui/src/window.rs`.
        windowReady().catch((error: unknown) => {
            // A browser without Tauri underneath it - `vite` opened directly, which is how the screens
            // are worked on - where there is no window to show and nothing has gone wrong.
            report("Failed to report that the window has something to show", error);
        });
    }, []);

    // Annotated rather than inferred: a component body that only runs an effect returns nothing, and
    // `void` is not a `ReactNode`. Biome rewrites an explicit `return undefined` to a bare `return`,
    // so the type is stated on the signature where nothing will trim it.
    return;
};

ReactDOM.createRoot(container).render(
    <StrictMode>
        <AppProviders>
            {/*
             * Inside the providers, so the recovery screen has the toaster and the tooltips. Around
             * `App` alone, so that a crash on the first render still commits the recovery screen and
             * `RevealWindow` beside it: the window is shown with a way to recover on it, rather than
             * left hidden until Rust's grace period and then blank.
             */}
            <ErrorBoundary>
                <App />
            </ErrorBoundary>

            {/*
             * Beside `App` rather than written into it, because it is not part of the application - it
             * is the last line of its startup, and `App` is rendered by four test files that have no
             * window. Last, so it commits with the tree above it rather than ahead of it.
             */}
            <RevealWindow />
        </AppProviders>
    </StrictMode>,
);
