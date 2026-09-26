import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import { FaceBoxes } from "@/features/faces/FaceBoxes";
import { useFaceSelection } from "@/features/faces/useFaceSelection";
import { framedDimensions } from "@/ipc/crop";
import { renditionFor } from "@/ipc/images";
import { FACES_DIALOG_BOUND } from "@/lib/constants";
import { useImageCrop } from "@/stores/crop";
import { useImageFaces } from "@/stores/faces";
import { useCurrentFile } from "@/stores/files";

/**
 * What the dialog's own chrome takes out of the window's **height**, in pixels.
 *
 * 32 of inset at the top and 32 at the bottom, 1 of border on each of those edges, the 48px title bar
 * and the rule under it, the 52px footer and the rule over it, and the 2px the picture is padded by
 * top and bottom. Every one of those is a number written in the markup below or in `DialogTitleBar`,
 * so this is the one place they are added up.
 */
const VERTICAL_CHROME = 64 + 2 + 49 + 53 + 4;

/** What the picture's own frame takes out of the width: the two borders and the two 2px pads. */
const PICTURE_FRAME = 2 + 4;

/** And the whole of the width, with the 32px inset on each side. */
const HORIZONTAL_CHROME = 64 + PICTURE_FRAME;

/**
 * How tall the picture may be, and how wide the dialog therefore is, for a photograph of these
 * framed dimensions.
 *
 * Both are the same `min` of the two edges - the window's height less the chrome, and the window's
 * width less the chrome converted through the photograph's own proportions - so whichever edge binds,
 * binds in both, and the picture is the photograph's shape at whichever size that leaves. Both are
 * written in viewport units alone.
 */
export const pictureFit = (width: number, height: number) => ({
    // Written against the viewport rather than against the parent, which is the whole of why it works:
    // asking the picture to fill the dialog's height while the dialog takes its width from the picture
    // is circular, and a browser resolves that circle by dropping the aspect. `100vh` and `100vw` are
    // definite before any of this lays out, so there is no circle to resolve.
    //
    // Exported for its own test: a browser and jsdom serialize a math expression differently, so the
    // arithmetic is pinned here where it is a string rather than through a style attribute where it is
    // whichever parser read it.
    picture: `min(100vh - ${VERTICAL_CHROME}px, (100vw - ${HORIZONTAL_CHROME}px) * ${height} / ${width})`,
    dialog: `calc(min(100vw - ${HORIZONTAL_CHROME}px, (100vh - ${VERTICAL_CHROME}px) * ${width} / ${height}) + ${PICTURE_FRAME}px)`,
});

/**
 * Screen 13b: the current photograph, framed, with a box over each face found in it, and an Apply
 * that says how many of them the run will restore.
 *
 * **The photograph fits in CSS, and no JavaScript measures the viewport.** The picture box is sized
 * from `100vh` and `100vw` less the chrome above, so it is the smaller of the two that binds and the
 * dialog is exactly as wide as the picture plus its frame - see {@link pictureFit}.
 *
 * **The picture is the framed rendition**, bounded by {@link FACES_DIALOG_BOUND}.
 *
 * **Apply commits; the close box, Escape and a click outside all discard**: the working copy is thrown
 * away and the store is never touched.
 */
export const SelectFacesDialog = ({ open, onClose }: { open: boolean; onClose: () => void }) => {
    /*
     * Fitting in CSS removes the whole of the reference's fitting apparatus - its
     * `window.innerWidth`/`innerHeight` state, its resize listener and the coalescing to one update per
     * animation frame - and it is the other half of what makes the boxes assertable, since a percentage
     * is in the style attribute where a measurement is not.
     *
     * Every way out of this one is an answer, unlike Crop/Rotate, which refuses to be dismissed by a
     * stray click because a framing is work in progress. `useFaceSelection` holds the working copy.
     */
    const { t } = useTranslation();

    // Read here rather than threaded in, as `EnhancementRow` reads the framing it counts faces at: the
    // row carries an identity and this needs the record - its dimensions as well as its pixels - and
    // passing one down through the list would be a second copy of a lookup the store already answers.
    const file = useCurrentFile();
    const crop = useImageCrop(file?.identity);
    const faces = useImageFaces(file?.identity, crop) ?? [];

    const { skipped, toggle, apply } = useFaceSelection(file?.identity, faces, open);

    /*
     * What the rendition decoded to, for the one photograph `framedDimensions` cannot answer for: one
     * whose header this application could not read and which carries no framing. Exact whenever the
     * photograph is under the bound above, and the best available otherwise - and it is a corner of a
     * corner, since the file decoded but did not measure.
     */
    const [natural, setNatural] = useState<{ width: number; height: number }>();

    const { width = natural?.width, height = natural?.height } = framedDimensions(file, crop);

    // The working copy is the faces left unchosen, so every other face is chosen.
    const chosen = faces.filter((face) => !skipped.has(face.key)).length;

    /*
     * Absent only for a photograph this application could not measure and whose rendition has not
     * decoded yet. The dialog then sizes itself to its content for the frame or two that lasts.
     */
    const fit = width && height ? pictureFit(width, height) : undefined;

    const commit = () => {
        apply();
        onClose();
    };

    return (
        <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
            <DialogContent
                showCloseButton={false}
                aria-describedby={undefined}
                /*
                 * `top-8 bottom-8` with the centring undone vertically: the generated content is
                 * positioned at 50/50 and pulled back by half its own size, so `top-8` replaces the
                 * anchor and the translate is reduced to the horizontal half that still centres it.
                 * `max-w-none` twice for the reason `SettingsDialog` gives - `twMerge` does not treat
                 * a bare utility and its `sm:` variant as conflicting, so the 512px cap survives
                 * otherwise.
                 *
                 * `min-w-min` is the floor under the fitted width: a tall, narrow photograph would
                 * otherwise make the dialog narrower than its own footer. It is the footer's
                 * `min-content`, so the floor is whatever the footer actually needs in the language
                 * being spoken rather than a number measured once in English.
                 */
                className="fixed top-8 bottom-8 left-1/2 flex h-auto w-auto max-w-none min-w-min -translate-x-1/2 translate-y-0 flex-col gap-0 overflow-hidden rounded-xl border border-border bg-card p-0 sm:max-w-none"
                {...(fit && { style: { width: fit.dialog } })}
            >
                <DialogTitleBar title={t("faces.selectFaces")} />

                {/*
                 * The 2px the mockup pads the picture by, over the canvas's own black rather than the
                 * card - so the photograph sits on a surface rather than on the dialog's chrome.
                 */}
                <div className="flex min-h-0 flex-1 items-center justify-center bg-background p-0.5">
                    {/*
                     * Sized by its **height** alone: the width follows from the aspect, rather than
                     * from the dialog. A `w-full` here is wrong in exactly one case and it is the
                     * case the floor above creates - a tall, narrow photograph in a short window,
                     * where the dialog is as wide as its footer needs and the picture would stretch
                     * to it and stop being the photograph's shape.
                     */}
                    <div
                        className="relative h-full"
                        data-slot="faces-picture"
                        {...(fit && { style: { height: fit.picture, aspectRatio: `${width} / ${height}` } })}
                    >
                        {/*
                         * The dialog opens at once and the photograph fills in, as Crop/Rotate's does
                         * and for its reason: a large scan is a decode, an encode and a transfer away
                         * even bounded, and a control that does nothing visible for a second reads as
                         * one that did not work. The boxes are drawn meanwhile - they are percentages,
                         * so they do not wait on pixels either.
                         *
                         * Framed because the faces are in the framed photograph's pixels, so a chooser
                         * drawing the unframed file would put every box in the wrong place and a face
                         * the crop removed over nothing at all. Bounded for the trade
                         * `FACES_DIALOG_BOUND` describes.
                         */}
                        <img
                            alt={t("common.previewAlt")}
                            src={renditionFor(file, FACES_DIALOG_BOUND, crop)}
                            onLoad={(event) =>
                                setNatural({
                                    width: event.currentTarget.naturalWidth,
                                    height: event.currentTarget.naturalHeight,
                                })
                            }
                            className="size-full object-contain"
                        />

                        <FaceBoxes
                            {...(width && { width })}
                            {...(height && { height })}
                            faces={faces}
                            skipped={skipped}
                            onToggle={toggle}
                        />
                    </div>
                </div>

                <div className="flex h-13 flex-none items-center justify-between gap-4 border-t border-border px-3">
                    <span className="text-foreground-dim text-xs">{t("faces.hint")}</span>

                    <Button className="h-8 px-3.5 font-semibold text-[13px]" onClick={commit}>
                        {t("faces.apply", { count: faces.length, enabled: chosen })}
                    </Button>
                </div>
            </DialogContent>
        </Dialog>
    );
};
