import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { FileMenuTrigger, FileOptionsMenu, NAVBAR_ANCHOR } from "@/features/files/FileOptionsMenu";
import { AboutDialog } from "@/features/navbar/AboutDialog";
import { NavbarDimensions } from "@/features/navbar/NavbarDimensions";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { useSettingsDialog } from "@/features/settings/useSettingsDialog";
import { appVersion, isOutdated } from "@/ipc/app";
import { openLink } from "@/ipc/links";
import { isMacOs } from "@/ipc/os";
import { APP_NAME } from "@/lib/constants";
import { track } from "@/lib/faro";
import { fileName } from "@/lib/paths";
import { report } from "@/lib/report";
import { cn } from "@/lib/utils";
import { useCurrentFile } from "@/stores/files";

/**
 * The strip across the top of the window, and on macOS the window's title bar.
 *
 * With an image open it also reports which one: its file name beside the application's, the file
 * options menu for it, and on the right its size - see `NavbarDimensions`. All three are absent with
 * nothing open.
 */
export const Navbar = () => {
    const { t } = useTranslation();
    const [version, setVersion] = useState<string>();
    const file = useCurrentFile();
    // The dialog's open state and its draft, which live and die together - see `useSettingsDialog`.
    // The navbar presses the button and renders the surface; it does not operate the settings store.
    const { open: settingsOpen, openSettings, close: closeSettings, draft } = useSettingsDialog();
    // Held here rather than in a hook like Settings': About has no draft, only whether it is showing.
    const [aboutOpen, setAboutOpen] = useState(false);
    // Whether a newer release is published. One boolean with one reader, so local state rather than a
    // store; absent until the check has answered, and the button with it.
    const [outdated, setOutdated] = useState(false);

    useEffect(() => {
        // No error branch: a rejection here means the IPC bridge is broken, and a rejected promise in
        // the console says so better than a caught one rendered where a version number should be.
        appVersion().then(setVersion);
    }, []);

    useEffect(() => {
        // A failed check answers `false` on the Rust side, which has logged why, so a rejection here can
        // only be a broken IPC bridge. Caught all the same: an unhandled one would say nothing more, and
        // the button stays absent either way.
        isOutdated()
            .then(setOutdated)
            .catch((error: unknown) => report("the update check could not be asked", error));
    }, []);

    const openReleases = () => {
        // Whether people act on the notice. That one was shown is the backend's `is_outdated` span.
        track("update_opened", {});

        // Logged rather than toasted, as the About dialog's links are, and the button stays: only a
        // machine with no browser registered for `https` can fail here.
        openLink("releases").catch((error: unknown) => report("the releases link could not be opened", error));
    };

    return (
        <header
            // What makes this the title bar. `titleBarStyle: "Overlay"` hands the title bar's area to the
            // webview, so without this the window has nothing left to drag it by; Tauri's handler also
            // restores double-click-to-zoom. The attribute is on this container and nothing inside it:
            // it applies to children too unless they handle the event themselves, so the buttons below
            // sit in their own elements and fire rather than drag. It goes on every platform - elsewhere
            // it costs nothing and makes the navbar behave like the header bar it looks like.
            data-tauri-drag-region
            className={cn(
                "flex h-12 flex-none items-center gap-3 border-b border-border bg-card px-3",
                // The traffic lights are drawn over this strip on macOS, so the name starts clear of
                // them. 86px is the Wails app's value. Nowhere else: Windows and Linux keep their own
                // title bar and have nothing to leave room for.
                isMacOs() && "pl-[86px]",
            )}
        >
            <span className="font-medium text-base">{APP_NAME}</span>

            {/*
             * The file's own name rather than its path: the navbar answers "which photograph am I
             * looking at", and a path deep enough to push Settings off the window answers it worse.
             * Absent with nothing open, which is what the design draws.
             */}
            {file && (
                <div className="ml-2 flex h-6 items-center gap-1.5">
                    <Separator orientation="vertical" className="mr-2 data-[orientation=vertical]:h-5" />
                    <span className="text-xs text-muted-foreground">{fileName(file.path)}</span>

                    {/*
                     * The same menu a thumbnail opens - one menu offered twice rather than two menus -
                     * acting on the current image, which is the image the navbar names.
                     *
                     * Anchored below its control and centred on it, which is what the design draws and
                     * what the reference implementation does for the same menu - unlike the
                     * thumbnail's, which diverges. Not portalled anywhere in particular: nothing in the
                     * navbar folds when something outside it is pressed, so Radix's default target is
                     * right here.
                     *
                     * Sized to the glyph like the thumbnail's, so the two controls are the same
                     * control - see `FILE_MENU_GLYPH`, which is where that number is explained.
                     */}
                    <FileOptionsMenu file={file} anchor={NAVBAR_ANCHOR}>
                        {/*
                         * `ml-2` on top of the row's own `gap-1.5`: the design sets the control
                         * further off the file name than the name is off the separator before it, so
                         * the name reads as one label rather than as a label with a control stuck to it.
                         */}
                        <FileMenuTrigger name={fileName(file.path)} className="ml-2 text-muted-foreground" />
                    </FileOptionsMenu>
                </div>
            )}

            <div className="flex-1" />

            {/*
             * Its own component because it reads the enhancement stack and the crop store, decides
             * which of three numbers describes what the user is working on, and compares all three on
             * hover. See `NavbarDimensions`, including why it can be absent.
             */}
            {file && <NavbarDimensions file={file} />}

            <Button variant="ghost" size="sm" onClick={openSettings}>
                {t("settings.title")}
            </Button>

            <SettingsDialog open={settingsOpen} close={closeSettings} draft={draft} />

            <Button variant="ghost" size="sm" onClick={() => setAboutOpen(true)}>
                {t("navbar.about")}
            </Button>

            <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} {...(version && { version })} />

            {/*
             * Rendered only once it has arrived rather than behind a placeholder: `version` crosses
             * the IPC boundary, so it is absent for the first tick, and a dash or a spinner in its
             * place would be a visible flicker for a string nobody is waiting on.
             *
             * `mr-2` is the design's own `margin-right:8px` on this span, carried over as declared: it
             * separates the version from the "Update Available" button that follows it. With no newer
             * release that button is absent, and the margin reads as an 8px inset from the window edge
             * on top of the header's 12px - which the design wants too.
             */}
            {version && (
                <span className="mr-2 font-mono text-xs text-foreground-faint">{t("navbar.version", { version })}</span>
            )}

            {/*
             * Shown only once the check has answered that a newer release is published, and nothing in
             * its place otherwise. `ml-1` is the design's `margin-left:4px`; the pulse is the
             * reference's, which the design, a still, cannot show.
             */}
            {outdated && (
                <Button size="sm" className="ml-1 animate-pulse" onClick={openReleases}>
                    {t("navbar.updateAvailable")}
                </Button>
            )}
        </header>
    );
};
