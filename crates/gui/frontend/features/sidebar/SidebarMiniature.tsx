import { useState } from "react";
import { Crop } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { CropRotate } from "@/features/crop/CropRotate";
import { useCurrentRendition } from "@/hooks/useCurrentRendition";
import { useTransformStore } from "@/stores/transform";

/**
 * The sidebar's preview area: a miniature of the current image, the rectangle showing which part of
 * it the canvas is on, and the way in to Crop/Rotate.
 *
 * **The crop button is drawn with the miniature and absent without one.**
 */
export const SidebarMiniature = () => {
    const { t } = useTranslation();
    const rendition = useCurrentRendition();
    const viewport = useTransformStore((state) => state.viewport);
    // Here rather than in `Sidebar`, which is what the reference's `SidebarImage` is: the column itself
    // has nothing to do with whether a dialog is open, and a `useState` for that in the sidebar would be
    // the first piece of state in a component that is otherwise all layout. A plain `useState` - there
    // is no draft to keep, so it needs no `useSettingsDialog`-shaped hook.
    const [cropping, setCropping] = useState(false);

    return (
        <div className="relative flex h-36 flex-none items-center justify-center border-b border-border">
            {rendition ? (
                <>
                    {/*
                     * The miniature is not drawn from the URL the canvas is drawing: it is a
                     * rendition of its own, framed and bounded at `THUMBNAIL_BOUND`, because outside
                     * Windows the webview's cache does not answer a second reader of the canvas's
                     * URL and each `<img>` would decode the photograph at full size. See
                     * `useCurrentRendition`.
                     *
                     * **The `<img>` element's box is the photograph's box**, inside a wrapper that
                     * hugs it, which is what the rectangle needs: `h-full` would make the element
                     * 144px tall and up to 256px wide *whatever the photograph's shape*, with the
                     * browser letterboxing inside it. A 3000x1000 photograph is drawn 256x85 in a
                     * 256x144 element, so a rectangle positioned in percentages of that element would
                     * be stretched over dead space by 40% of the panel's height. The reference's
                     * miniature is bounded the same way for the same reason.
                     */}
                    <div className="relative" data-slot="sidebar-miniature">
                        <img
                            src={rendition}
                            alt={t("sidebar.zoomCropAlt")}
                            className="block max-h-36 max-w-full"
                            data-slot="sidebar-preview"
                        />

                        {/*
                         * Which part of the photograph the canvas is showing, straight from what the
                         * enhanced pane publishes - positioned and sized in percentages of a box that
                         * is exactly the photograph's.
                         *
                         * It surrounds the whole miniature while the image is drawn whole, which is
                         * the true answer rather than a special case: all of the photograph is being
                         * shown. The design draws it inset on every screen, including the ones at 1x,
                         * but that is a static mockup rather than a rule.
                         */}
                        {viewport && (
                            <div
                                style={{
                                    left: `${viewport.x * 100}%`,
                                    top: `${viewport.y * 100}%`,
                                    width: `${viewport.width * 100}%`,
                                    height: `${viewport.height * 100}%`,
                                }}
                                className="pointer-events-none absolute box-border border-2 border-white"
                                data-slot="sidebar-viewport"
                            />
                        )}
                    </div>

                    {/*
                     * Drawn only with the miniature, which is a carve-out from the shell's standing
                     * rule that a control acting on an image is disabled rather than hidden. That rule
                     * exists to keep the frame's shape as an image loads, and this area is not part of
                     * the frame's shape: with nothing open it is not a control strip with an empty box
                     * in it but a sentence saying there is no preview. The mockup and the reference
                     * agree, and `gui-shell` records it as the one exception.
                     *
                     * Over the *panel's* lower trailing corner rather than the miniature's, which is
                     * where screen 05 puts it and is the robust reading of the two: a 3000x1000
                     * photograph is drawn 256x85 in this 256x144 box, and a button pinned to the
                     * picture's own corner would float a third of the way up the panel.
                     */}
                    <Button
                        type="button"
                        variant="outline"
                        size="icon-sm"
                        aria-label={t("crop.open")}
                        onClick={() => setCropping(true)}
                        className="absolute right-3 bottom-3 border-input bg-secondary dark:bg-secondary [&_svg:not([class*='size-'])]:size-4.5"
                        data-slot="sidebar-crop"
                    >
                        <Crop />
                    </Button>

                    <CropRotate open={cropping} onClose={() => setCropping(false)} />
                </>
            ) : (
                /*
                 * Without an image the words are padded rather than flush, for the reason the Wails
                 * app records: this is a full sentence, and in a language whose wording runs longer
                 * than English it renders edge-to-edge on one line - or clips - without room to wrap.
                 */
                <p className="px-6 text-center text-[13px] text-foreground-faint">{t("sidebar.noPreview")}</p>
            )}
        </div>
    );
};
