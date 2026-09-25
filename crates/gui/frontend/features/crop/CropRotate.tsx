import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { AspectRatios } from "@/features/crop/AspectRatios";
import { CropDimensions } from "@/features/crop/CropDimensions";
import { ImageCropper } from "@/features/crop/ImageCropper";
import { RotateControls } from "@/features/crop/RotateControls";
import { useCropController } from "@/features/crop/useCropController";
import { DOTTED_SURFACE } from "@/lib/canvas";
import { cn } from "@/lib/utils";

/**
 * Screen 18: the Crop/Rotate surface.
 *
 * Near-fullscreen - `inset-8` over the window, which is the 32px the design insets it by on every
 * side.
 *
 * **A click outside does not dismiss it.** Escape and the close box do, and no way out writes
 * anything.
 *
 * **The surface behind the photograph is dotted, always**, whatever the canvas is set to.
 *
 * The controller holds everything else. This file is layout.
 */
export const CropRotate = ({ open, onClose }: { open: boolean; onClose: () => void }) => {
    const { t } = useTranslation();
    const controller = useCropController(open, onClose);

    // Escape and the close box both go through Radix's `onOpenChange`, so there is one way out rather
    // than two that could come to differ. None of them writes anything: there is no draft to unwind.
    return (
        <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
            <DialogContent
                showCloseButton={false}
                aria-describedby={undefined}
                // Pointer-down outside is ignored; Escape is not. As the settings dialog refuses the
                // same and for the same reason: a stray click is not an answer to a framing the user
                // has been working on.
                onPointerDownOutside={(event) => event.preventDefault()}
                onInteractOutside={(event) => event.preventDefault()}
                /*
                 * `inset-8` with the centring undone. The generated content is positioned at 50/50
                 * and pulled back by half its own size, which for a box defined by its insets would
                 * push it off the bottom-right by half the window; `top-8 left-8` replaces the
                 * anchor and `translate-none` removes the correction. `sm:max-w-none` as well as
                 * `max-w-none` for the reason `SettingsDialog` gives: `twMerge` does not treat a bare
                 * utility and its `sm:` variant as conflicting, so the 512px cap survives otherwise.
                 */
                className="fixed inset-8 flex h-auto w-auto max-w-none translate-none flex-col gap-0 overflow-hidden rounded-xl border border-border bg-card p-0 sm:max-w-none"
            >
                <DialogTitleBar title={t("crop.title")} />

                {/*
                 * Dotted whatever the canvas is set to, rather than `useCanvasSurface`'s choice. The
                 * particle field is a *canvas* backdrop - drifting points behind a photograph the user
                 * is looking at - and this is a work surface where the thing being judged is the
                 * position of an edge against the background behind it. A static grid is a ruler; a
                 * moving field competes with the rectangle being placed. The value is
                 * `lib/canvas.tsx`'s, so there is one definition of the dotted surface rather than a
                 * copy of the gradient here.
                 */}
                <div className={cn("relative flex min-h-0 flex-1", DOTTED_SURFACE)}>
                    <div className="flex min-w-0 flex-1 flex-col gap-4 overflow-hidden p-8">
                        {/*
                         * The dialog opens at once and the photograph fills in, rather than the whole
                         * surface waiting on the rendition the way the reference's does. A large scan is
                         * a decode, an encode and a transfer away even bounded, and a control that does
                         * nothing visible for a second reads as one that did not work.
                         *
                         * The pane is left empty meanwhile rather than captioned: every string here is a
                         * catalogue key in thirteen locales, and the catalogue has none for this. The
                         * dotted surface is showing underneath, so it is not a blank rectangle.
                         */}
                        <div className="flex min-h-0 flex-1 items-center justify-center">
                            {controller.source && (
                                <ImageCropper
                                    ref={controller.cropper}
                                    src={controller.source.url}
                                    {...(controller.aspectRatio !== undefined && {
                                        aspectRatio: controller.aspectRatio,
                                    })}
                                    onChange={controller.syncDimensions}
                                    onReady={controller.onReady}
                                />
                            )}
                        </div>

                        <RotateControls
                            rotation={controller.fineRotation}
                            onRotationChange={controller.onRotationChange}
                            onRotate90={controller.onRotate90}
                            onFlipHorizontal={controller.onFlipHorizontal}
                            onFlipVertical={controller.onFlipVertical}
                            onReset={controller.onReset}
                        />
                    </div>

                    <div className="flex w-64 flex-none flex-col gap-2 overflow-hidden border-l border-border bg-card p-4">
                        <span className="text-[13px] text-foreground">{t("crop.aspectRatio")}</span>

                        <AspectRatios selected={controller.ratio} onSelect={controller.onSelectRatio} />

                        <div className="my-2 h-px flex-none bg-border" />

                        <span className="text-[13px] text-foreground">{t("navbar.dimensions.title")}</span>

                        <CropDimensions
                            width={controller.width}
                            height={controller.height}
                            onWidthCommit={controller.onWidthCommit}
                            onHeightCommit={controller.onHeightCommit}
                            onSwap={controller.onSwap}
                        />

                        <p className="mt-6 text-center text-[13px] leading-relaxed text-muted-foreground text-pretty">
                            {t("crop.zoomHint")}
                        </p>

                        <div className="flex-1" />

                        <div className="flex flex-none gap-3">
                            <Button variant="secondary" className="h-9 flex-1 font-normal" onClick={onClose}>
                                {t("common.cancel")}
                            </Button>
                            <Button className="h-9 flex-1 font-normal" onClick={controller.onApply}>
                                {t("common.apply")}
                            </Button>
                        </div>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
};
