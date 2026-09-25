import { FolderOpen } from "lucide-react";
import { Trans, useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { useOpenImages } from "@/hooks/useOpenImages";

/**
 * What the canvas shows while nothing is open: how to supply an image, by either route.
 *
 * Both routes work. Browse images opens the platform's picker through the hook the drawer's Add
 * images uses, and the drag-and-drop half of the invitation is listened to by the shell - anywhere on
 * the window, not over this element, because that is how Tauri delivers a drop.
 */
export const PreviewEmpty = () => {
    const { t } = useTranslation();
    const openImages = useOpenImages("empty");

    return (
        <div className="flex size-full flex-col items-center justify-center">
            {/*
             * The one place the app's icon stroke is overridden: the design draws this at 72px and
             * stroke 1.25, where the provider's 1.5 reads heavy at that size.
             */}
            <FolderOpen className="size-18 text-primary" strokeWidth={1.25} />

            <div className="mt-4 mb-4.5 flex flex-col items-center gap-2.5 text-center">
                {/*
                 * `<Trans>` rather than `t()`: the catalogue's value carries a `<br/>`, because the
                 * line break is part of the centred two-line layout and a translator has to be free
                 * to move or drop it. `<br/>` is one of i18next's default `transKeepBasicHtmlNodesFor`
                 * nodes, so it survives; `t()` would render it as literal text.
                 */}
                <p className="text-[15px]/[1.5]">
                    <Trans i18nKey="preview.empty.title" />
                </p>

                <span className="text-xs font-medium tracking-wider text-muted-foreground">{t("common.or")}</span>
            </div>

            <Button onClick={openImages}>{t("preview.empty.browse")}</Button>
        </div>
    );
};
