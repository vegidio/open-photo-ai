import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import icon from "@/assets/icon.avif";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { type Link, openLink } from "@/ipc/links";
import { APP_COPYRIGHT, APP_LINKS, APP_NAME } from "@/lib/constants";
import { report } from "@/lib/report";

/** The links in the order the design draws them, left to right. */
const LINKS: (keyof typeof APP_LINKS)[] = ["repository", "website"];

/**
 * Screen 19: what this application is, which build is running, and who made it.
 *
 * The version and the copyright line are `select-text` - the window is `user-select: none` - because
 * they are what a user copies into a bug report. Nothing else here is.
 *
 * Dismissed by its close control, Escape and a click outside, all three through Radix's `onOpenChange`.
 */
type AboutDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    /** The running build's version, as the navbar fetched it; absent until it has arrived. */
    version?: string;
};

export const AboutDialog = ({ open, onOpenChange, version }: AboutDialogProps) => {
    const { t } = useTranslation();

    const follow = (link: Link) => {
        // Logged rather than toasted, as the reference leaves it: only a machine with no browser
        // registered for `https` can fail here, and the dialog stays as it was.
        openLink(link).catch((error: unknown) => report(`the ${link} link could not be opened`, error));
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent
                showCloseButton={false}
                aria-describedby={undefined}
                // `sm:max-w-none` as well as `max-w-none`, for the reason `SettingsDialog` gives.
                className="w-96 max-w-none gap-0 overflow-hidden rounded-xl bg-card p-0 shadow-[0_24px_60px_rgb(0_0_0/0.7)] sm:max-w-none"
            >
                <DialogTitleBar title={t("navbar.about")} />

                <div className="flex flex-col items-center gap-4 px-6 pt-2.5 pb-7">
                    <img src={icon} alt={t("navbar.about_dialog.iconAlt")} className="size-36" />

                    <div className="flex flex-col items-center gap-1">
                        <span className="font-bold text-xl">{APP_NAME}</span>

                        {/* Only once it has arrived, as the navbar draws it - never behind a placeholder. */}
                        {version && (
                            <span className="select-text text-[13px] text-muted-foreground">
                                {t("navbar.about_dialog.version", { version })}
                            </span>
                        )}
                    </div>

                    <div className="mt-1.5 flex flex-col items-center gap-1.5">
                        <span className="select-text text-[13px] text-muted-foreground">{APP_COPYRIGHT}</span>

                        <div className="flex items-center gap-2.5">
                            {LINKS.map((link, index) => (
                                <Fragment key={link}>
                                    {index > 0 && <span aria-hidden className="h-3 w-px bg-input" />}

                                    {/*
                                     * A button drawn as a link rather than an `<a href>`: an anchor in the
                                     * webview navigates the application window whenever a click reaches it
                                     * unhandled - a middle click, a modifier key - and the reference's
                                     * `href="#"` changes the location on every click.
                                     */}
                                    <button
                                        type="button"
                                        className="cursor-pointer rounded-sm text-[13px] text-link hover:text-link-hover hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                                        onClick={() => follow(link)}
                                    >
                                        {APP_LINKS[link]}
                                    </button>
                                </Fragment>
                            ))}
                        </div>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
};
