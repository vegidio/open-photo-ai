import { useEffect, useState } from "react";
import { type ImageRecord, renditionFor } from "@/ipc/images";
import { useImageCrop } from "@/stores/crop";
import { useCurrentFile } from "@/stores/files";

/**
 * The current image, held back until its own pixels are ready to be painted.
 *
 * The canvas draws the photograph at a size it has worked out itself - `fitInside` against the pane,
 * with no `object-contain` - so the `<img>`'s box comes from the *record*, which changes the instant
 * the strip is clicked, while the element goes on showing the previous photograph until the new
 * rendition has decoded. Between those two moments the old photograph is drawn into the new one's
 * box: a landscape squeezed into a portrait for as long as the decode takes, which on a first visit
 * to a file is long enough to read as the image distorting before it changes.
 *
 * Holding the record back until the pixels exist makes the two halves one event. Everything the
 * canvas keys off the file - the source, the fitted size, the identity its transform is stored under
 * - then changes on the same render, so the photograph on screen is never drawn to another
 * photograph's shape.
 *
 * **Only the canvas waits.** The navbar, the sidebar and the strip's own outline key off
 * {@link useCurrentFile} and move at once, because they are the acknowledgement that the click
 * landed; a strip that took a beat to mark what had been chosen would be a worse trade than the one
 * this fixes.
 *
 * It mounts settled, on whatever is current: there is no photograph on screen yet at that point, so
 * there is nothing for the swap to be atomic with, and waiting would only delay the first image.
 *
 * **The framing is part of what is waited for**, not just the photograph. A crop changes the shape of
 * the pane's box in the same render it changes the pixels, so the two have to arrive together for the
 * same reason a change of image does - see the crop the body reads.
 */
export const useSettledFile = (): ImageRecord | undefined => {
    const current = useCurrentFile();
    /*
     * The framing the canvas will draw, because what is preloaded has to be the URL the canvas asks
     * for: the pane sizes its box from the crop's own rectangle, so waiting on the *uncropped*
     * rendition would settle the record - and change the box - a decode before the framed pixels
     * exist, which is the distortion this hook was written to remove.
     */
    const crop = useImageCrop(current?.identity);
    const [settled, setSettled] = useState(current);

    useEffect(() => {
        // Already showing what was asked for - the mount, and the render the swap below causes. There
        // is nothing to preload for a photograph the canvas is drawing.
        if (settled === current) return;

        const settle = () => setSettled(current);
        const source = renditionFor(current, 0, crop);

        // A file whose bytes could not be read has no rendition to wait for. It is drawn as a broken
        // image either way, and holding the record back would leave the previous photograph on screen
        // under the new one's name.
        if (source === undefined) {
            settle();
            return;
        }

        const preload = new Image();

        // `decode()` resolves once the bitmap can be painted, which is the guarantee this needs and
        // the one `load` does not give - a decoded-on-paint image would swap its box a frame before
        // its pixels. Absent in jsdom, and in any webview old enough to lack it: there the swap is
        // immediate, which is exactly the behaviour this hook replaced.
        if (typeof preload.decode !== "function") {
            settle();
            return;
        }

        let live = true;

        preload.src = source;
        // Settled on failure as well as on success, for the reason the missing-identity case gives:
        // a rendition the protocol refuses still has to reach the canvas.
        preload.decode().then(
            () => live && settle(),
            () => live && settle(),
        );

        return () => {
            live = false;
        };
    }, [current, settled, crop]);

    return settled;
};
