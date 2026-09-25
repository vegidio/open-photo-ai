import { setAnalytics } from "@/ipc/analytics";
import { pauseFaro } from "@/lib/faro";
import { report } from "@/lib/report";
import { useSettingsStore } from "@/stores/settings";

/**
 * Hands `enabled` to Rust, reporting and dropping a rejection.
 *
 * An opt-out pauses the window's own sending first, synchronously and before Rust is asked, so not even
 * the batch Faro holds unsent leaves: nothing about the opt-out, and nothing after it, is sent from
 * either half. An opt-in does nothing to Faro. The backend waits for the next launch, and so does the
 * window.
 */
const mirror = (enabled: boolean) => {
    if (!enabled) pauseFaro();

    setAnalytics(enabled).catch((error: unknown) => {
        // A browser without Tauri underneath it - `vite` opened directly - rejects every call. A failure
        // the command reached is recorded on the Rust side already, and `report` does not send it again;
        // one that never reached it, such as a broken bridge, is recorded by `report`.
        report("Failed to record the analytics choice", error);
    });
};

/**
 * Keeps Rust's copy of the Analytics choice in step with the settings store, which is where the user
 * made it.
 *
 * Sends the stored choice once, now, and again whenever it changes. Rust reads its copy at the next
 * launch, before this window exists, and stops sending the moment it is told the choice is off - so a
 * copy that went missing or disagrees is corrected on the first boot that sees it. The window stops
 * sending a moment earlier still: see `mirror`.
 *
 * Returns the unsubscribe.
 */
export const mirrorAnalytics = (): (() => void) => {
    // From the store's state rather than from the Save that changed it: `apply` is the only writer of
    // this field today, but a subscription sees every writer, including any added later, and keeps the
    // store itself free of IPC.
    mirror(useSettingsStore.getState().analytics);

    return useSettingsStore.subscribe((state, previous) => {
        if (state.analytics !== previous.analytics) mirror(state.analytics);
    });
};
