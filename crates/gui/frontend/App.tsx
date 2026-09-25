import { useCallback, useEffect } from "react";
import { Drawer } from "@/features/drawer/Drawer";
import { Navbar } from "@/features/navbar/Navbar";
import { Preview } from "@/features/preview/Preview";
import { SetupDialog } from "@/features/setup/SetupDialog";
import { TensorRTDialog } from "@/features/setup/TensorRTDialog";
import { Sidebar } from "@/features/sidebar/Sidebar";
import { initialize } from "@/ipc/setup";
import { track } from "@/lib/faro";
import { report } from "@/lib/report";
import { useEnhancementStore } from "@/stores/enhancements";
import { useSettingsStore } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";

/**
 * Sends `app_ready`: what this load runs with. A snapshot counts every user, where a change event
 * would count only those who changed something.
 */
const announceReady = () => {
    const { processor, language, background } = useSettingsStore.getState();

    track("app_ready", { processor, language, background, autopilot: useEnhancementStore.getState().autopilot });
};

/**
 * The window's frame: a navbar over a row of the canvas and the sidebar, with the image drawer drawn
 * across the bottom of the canvas.
 *
 * All four regions are present whatever is loaded: what they contain changes with the application's
 * state, and where each one sits does not.
 */
const App = () => {
    // No region takes its empty state from here. The two that ask read it from the store themselves:
    // the drawer's header through `useHasFiles`, and the sidebar's Add enhancement through
    // `useCurrentFile`, which leaves it unavailable until an image is open. The sidebar's export button
    // asks nothing - it is unavailable whether or not an image is open. A boolean computed here and
    // threaded through `Drawer`, which does not read it, would make the shell re-render on every
    // empty-to-not transition to tell a component something it can see.
    const status = useSetupStore((state) => state.status);
    const apply = useSetupStore((state) => state.apply);
    const succeeded = useSetupStore((state) => state.succeeded);
    const failed = useSetupStore((state) => state.failed);
    const retry = useSetupStore((state) => state.retry);
    const tensorrt = useSetupStore((state) => state.providers?.tensorrt === true);
    const tensorrtAsked = useSettingsStore((state) => state.tensorrtAsked);

    /** One attempt at starting the application, from the mount effect and from Try again alike. */
    const start = useCallback(() => {
        // One call site rather than two, because the two differ in nothing: the same command, the same
        // channel handler, the same two resolutions. It is also what makes the serialization in Rust
        // meaningful - a Try again pressed while an attempt is somehow still running is caught by
        // `Setup`'s lock, exactly as StrictMode's double mount is.
        //
        // `retry()` first, so the setup dialog is back the moment the button is pressed rather than
        // after the attempt's first report. On the mount call it does nothing, by its own guard: there
        // is no failure to clear, and `apply` is what moves an `idle` launch to `running` on the first
        // report of work - which is what keeps a launch with nothing to install showing no dialog.
        retry();

        // A fresh `Channel` per call, because `initialize` creates one. The previous attempt's is
        // dropped with nothing referencing it; a late report on it lands in a handler updating rows
        // the new plan is about to replace.
        initialize(apply).then(
            (providers) => {
                // Once per load: the first success is the one that moves the store to `ready`. A second
                // answer - StrictMode's double mount in development - finds it there already.
                const first = useSetupStore.getState().status !== "ready";
                succeeded(providers);
                if (first) announceReady();
            },
            (error: unknown) => {
                // Written to the log by Rust where it happened, so `report` only puts it in the webview's
                // console, for a developer; what a user reads is the failure dialog.
                report("Failed to initialize the application", error);
                failed(error);
            },
        );
    }, [apply, succeeded, failed, retry]);

    useEffect(() => {
        /*
         * The application starts itself, exactly as the Wails app does: nothing in it can be used
         * before initialization has finished, so making a user press something would be asking them
         * to confirm the only thing that can happen next.
         *
         * Nothing guards against this running twice. It genuinely does in development - StrictMode
         * double-mounts the effect - and a reloaded webview is a third call. The rule that one
         * initialization happens is Rust's `Setup`, deliberately: a module-scope flag here would not
         * see the reload, and would not see Try again either.
         *
         * The reports and the promise are two paths and neither waits for the other. A report that
         * lands after the promise resolves updates rows nobody is rendering, which is why the store
         * moves to `running` only from `idle`.
         */
        start();
    }, [start]);

    return (
        <div className="flex h-screen flex-col">
            <Navbar />

            <main className="flex min-h-0 flex-1 flex-row">
                {/* The positioning context for both the inset canvas and the drawer over it. */}
                <div className="relative min-w-0 flex-1 overflow-hidden">
                    <Preview />

                    <Drawer />
                </div>

                <Sidebar />
            </main>

            {/*
             * One dialog for both states, which is why the condition covers both: the status is the
             * whole of whether it is up, and the dialog itself reads the failure to decide which of
             * its two states to draw. `idle` covers both a launch that has not reported work yet and
             * a launch with nothing to install - the second of which shows no dialog at any point,
             * not even briefly.
             *
             * Try again returns it to its progress state in the same render it left the failed one:
             * `start` sets the status synchronously in the same React event.
             */}
            {(status === "running" || status === "failed") && <SetupDialog onRetry={start} />}

            {/*
             * Derived rather than held, like the setup dialog above: answering records the answer,
             * and the recorded answer is what takes it down. `ready` is only reached once the setup
             * dialog is gone, so the two are never up together, and a retry that succeeds reaches it
             * exactly as a first attempt does.
             */}
            {status === "ready" && tensorrt && !tensorrtAsked && <TensorRTDialog />}
        </div>
    );
};

export default App;
