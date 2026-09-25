import { renditionFor } from "@/ipc/images";
import { THUMBNAIL_BOUND } from "@/lib/constants";
import { useImageCrop } from "@/stores/crop";
import { useCurrentFile } from "@/stores/files";

/**
 * Where to draw the sidebar's miniature of the current image from, or `undefined` while there is
 * nothing to draw.
 *
 * **Bounded, at the strip's own `THUMBNAIL_BOUND`.** The miniature is drawn at `max-h-36` inside a
 * `w-64` aside - at most 256x144 CSS pixels - so the photograph at its own size would be tens of
 * megabytes fetched and decoded for an element that shows none of it.
 *
 * **It draws the framing**, so the miniature and the canvas show the same picture at two sizes. That
 * costs it the URL it used to share with the strip, which stays uncropped: a framed miniature is its
 * own rendition at the same bound. It is affordable because Rust reduces the photograph before it
 * turns it - the miniature pays a small warp rather than a full-resolution one, which is exactly the
 * cost the reference avoids by not cropping bounded renditions at all.
 *
 * This asked for no bound at all until the protocol's own cache note was written down. The reasoning
 * then was that a second reader of the *same* URL is free, because the response is served immutable
 * and the webview's cache answers it. That is true only on Windows: as `ipc/images.ts` records, a
 * custom scheme on macOS and Linux is served by a handler the resource cache does not sit in front
 * of, so each `<img>` issues its own request and does its own full-resolution decode. The bound is
 * what makes the miniature cheap where the cache does not exist.
 *
 * The miniature reads the *current* file rather than the settled one on purpose: it is a thumbnail
 * bounded by `max-h-36` with no box of its own to distort, so it has nothing to wait for.
 *
 * Absent for a file whose bytes could not be read. Such a file was never admitted, so nothing can
 * serve its pixels, and the window is refused them rather than given the wrong ones.
 */
export const useCurrentRendition = () => {
    const file = useCurrentFile();

    return renditionFor(file, THUMBNAIL_BOUND, useImageCrop(file?.identity));
};
