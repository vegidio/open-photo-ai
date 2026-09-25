import type { ReactNode } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { type SettingsData, settingsDefaults } from "@/stores/settings";
import { SettingsDraftProvider, type useDraftState } from "./draft.tsx";
import { EnhancementsPage } from "./EnhancementsPage.tsx";
import { ExportPage } from "./ExportPage.tsx";
import { GeneralPage } from "./GeneralPage.tsx";
import { PerformancePage } from "./PerformancePage.tsx";
import { SETTINGS_PAGES, type SettingsPageId } from "./pages.ts";
import { SettingsNav } from "./SettingsNav.tsx";

// **Total over `SettingsPageId`**, so a page added to the table and forgotten here is a compile error
// rather than a nav entry that opens onto nothing.
//
// The elements rather than functions returning them: these are module-scope `createElement` calls
// with no props, so there is nothing for a thunk to defer.
/** What each page draws, looked up by the page's own id. */
const PAGE_BODIES: Record<SettingsPageId, ReactNode> = {
    general: <GeneralPage />,
    performance: <PerformancePage />,
    enhancements: <EnhancementsPage />,
    export: <ExportPage />,
};

/** The given preferences' defaults, and nothing else - what one page's Reset writes. */
const defaultsFor = (keys: readonly (keyof SettingsData)[]): Partial<SettingsData> => {
    const defaults = settingsDefaults();

    return Object.fromEntries(keys.map((key) => [key, defaults[key]]));
};

/**
 * Everything inside the card: the nav, the page being viewed, and the footer.
 *
 * **Its own component so the page state lives inside the dialog's content**, which Radix unmounts on
 * close - that is what makes the surface open on General every time rather than on whichever page it
 * was last left on, with no effect to reset it.
 */
const SettingsSurface = ({ close, draft }: { close: (save: boolean) => void; draft: SettingsDraft }) => {
    const { t } = useTranslation();
    const [active, setActive] = useState<SettingsPageId>("general");
    const page = SETTINGS_PAGES.find(({ id }) => id === active) ?? SETTINGS_PAGES[0];

    return (
        <div className="flex min-h-0 flex-1">
            <SettingsNav active={active} onSelect={setActive} />

            <div className="flex min-w-0 flex-1 flex-col">
                {/*
                 * Scrolls rather than clips when a page outgrows the pane, which the longer locales can
                 * make the Enhancements table do. `scrollbar-thin` for the reason the drawer's strip
                 * carries it: the platform default is a wide bar against a 768px card.
                 */}
                <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto overflow-x-hidden p-6 scrollbar-thin">
                    <div className="flex flex-col gap-1.5">
                        <h2 className="m-0 font-semibold text-lg tracking-[-0.01em]">{t(page.titleKey)}</h2>
                        <p className="m-0 text-pretty text-[13px] text-muted-foreground leading-normal">
                            {t(page.descriptionKey)}
                        </p>
                    </div>

                    {PAGE_BODIES[page.id]}
                </div>

                <div className="flex flex-none items-center justify-between gap-3 border-border border-t py-3 pr-3 pl-6">
                    {/*
                     * Through the same `update` every control calls, so a reset is a draft by
                     * construction: Cancel discards it and Save applies it, with nothing special-cased
                     * on either path. Only this page's preferences, which is what `keys` is for.
                     *
                     * A quiet text action rather than a third filled button that would compete with
                     * Save. Always enabled: a reset of a page already at its defaults is harmless, and
                     * knowing that would mean comparing drafts deeply.
                     */}
                    <Button
                        variant="link"
                        className="h-auto p-0 font-normal text-[13px] text-muted-foreground hover:text-foreground"
                        onClick={() => draft.update(defaultsFor(page.keys))}
                    >
                        {t("settings.reset")}
                    </Button>

                    <div className="flex gap-3">
                        <Button variant="secondary" className="h-9 px-5" onClick={() => close(false)}>
                            {t("common.cancel")}
                        </Button>
                        <Button className="h-9 px-5" onClick={() => close(true)}>
                            {t("common.save")}
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    );
};

type SettingsDraft = ReturnType<typeof useDraftState>["draft"];

/**
 * Screens 16, 16b, 16c and 16d: the application's settings, as four pages shown one at a time.
 *
 * **Every preference on it is a draft until Save.** `useSettingsDialog` owns that draft and this
 * hands it to the pages, so this component draws the surface and reports which way the user left it,
 * and never touches the settings store. The draft lives outside the pages, which is what lets a change
 * survive moving between them.
 *
 * `close(true)` is Save and `close(false)` is Cancel, the close box and Escape. **A click outside
 * does not dismiss it.**
 */
export const SettingsDialog = ({
    open,
    close,
    draft,
}: {
    open: boolean;
    close: (save: boolean) => void;
    draft: SettingsDraft;
}) => {
    const { t } = useTranslation();

    return (
        <SettingsDraftProvider draft={draft}>
            {/*
             * Escape and the close box end it through `close(false)`, as Cancel does, which is what makes
             * "dismissing restores" true of every way out rather than of the button someone remembered.
             */}
            <Dialog open={open} onOpenChange={(next) => !next && close(false)}>
                <DialogContent
                    showCloseButton={false}
                    aria-describedby={undefined}
                    // Pointer-down outside is ignored; Escape is not. Everything on the surface is a draft,
                    // and a stray click is not an answer to "keep this or not" - which is the one
                    // interaction the reference also refuses, while accepting both the close control and
                    // Escape.
                    onPointerDownOutside={(event) => event.preventDefault()}
                    onInteractOutside={(event) => event.preventDefault()}
                    /*
                     * `sm:max-w-none` as well as `max-w-none`: the generated `DialogContent` carries
                     * `sm:max-w-lg`, and `twMerge` does not treat a bare utility and its `sm:` variant as
                     * conflicting - so without this the card comes up at 512px and the pages scroll
                     * sideways inside it. The design's 768x640 is the whole card, hence `p-0` and the
                     * metrics below rather than the dialog's own padding.
                     */
                    className="flex h-160 w-3xl max-w-none flex-col gap-0 overflow-hidden rounded-xl bg-card p-0 sm:max-w-none"
                >
                    <DialogTitleBar title={t("settings.title")} />

                    <SettingsSurface close={close} draft={draft} />
                </DialogContent>
            </Dialog>
        </SettingsDraftProvider>
    );
};
