import { type RefObject, useEffect, useState } from "react";
import type { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { describeImages, inputExtensions } from "@/ipc/images";
import { isWindows } from "@/ipc/os";
import { formatsOf } from "@/lib/events";
import { track } from "@/lib/faro";
import { extensionOf, fileName } from "@/lib/paths";
import { report } from "@/lib/report";
import { useFileStore } from "@/stores/files";

/**
 * What a drop's reported position has to be divided by to be the CSS pixels the DOM measures in.
 *
 * Tauri types the position as a `PhysicalPosition` on every platform, and on **Windows alone** it
 * actually is one: wry takes it from `ScreenToClient`, whose client coordinates are device pixels.
 * macOS reads it from `draggingLocation()`, which AppKit gives in points, and GTK reports logical
 * units as well - and `tauri-runtime-wry` relabels the pair as physical either way without scaling
 * it. So the type is right about one platform in three, and dividing by `devicePixelRatio` on a
 * Retina Mac halves every coordinate, which quietly pulls the sidebar and the drawer into whatever
 * rectangle is being tested.
 *
 * Read from wry 0.55's three `drag_drop.rs` implementations rather than from the type. Verified on
 * macOS, which is the only platform this series builds on; the Windows branch is what its source
 * says and stays unverified, as R6 records.
 */
const positionScale = () => (isWindows() ? window.devicePixelRatio : 1);

/**
 * Whether the pointer was over `element` when the files were let go of.
 *
 * The position is relative to the webview and the rectangle is relative to the viewport, which are
 * the same origin: `titleBarStyle: "Overlay"` in `tauri.conf.json` puts the frontend under the title
 * bar rather than below it, so the webview fills the window. A platform that inset the webview inside
 * the window would need that offset subtracted here.
 *
 * The right and bottom edges are exclusive, as a rectangle's own `contains` would have it: a drop at
 * `rect.bottom` is the first pixel of whatever is drawn under the canvas, not the last of the canvas.
 */
const isOver = (element: HTMLElement, position: PhysicalPosition) => {
    const { x, y } = position.toLogical(positionScale());
    const rect = element.getBoundingClientRect();

    return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
};

/**
 * Opens the images dropped on `target`, and names the files it will not open.
 *
 * **On `target`, not anywhere on the window.** Tauri delivers a drop as a window event carrying a
 * position rather than to an element, so the region that accepts one is a rule this hook enforces
 * itself: the position is converted into the DOM's own pixels and hit-tested against the element
 * the caller hands it. The caller hands it the region the preview draws its drop mark on - the surface
 * that says images can be dropped on it - so what the window offers and what it accepts are the same
 * rectangle, one element rather than two that could come to disagree. A drop over the navbar, the
 * sidebar or the drawer is ignored, exactly as the reference ignores it.
 *
 * **The drawer's strip is ignored whether the drawer is folded or not**, which is the question this
 * hook used to park here for a later slice. The preview canvas is inset by the folded drawer's header
 * permanently, and unfolding slides the body up *over* the canvas rather than resizing it - so the
 * canvas's own bottom edge sits inside an open drawer, and hit-testing against it would accept a drop
 * let go of on the thumbnails. The caller's answer is to hand over a region bottomed at the drawer's
 * top rather than the canvas's, so the thumbnails are outside the rectangle by construction. See
 * `Preview.tsx`, and design.md D7 of `add-gui-drop-beam`.
 *
 * **The partition is by extension, as the reference partitions it**, against the list the decoder
 * itself reports. Judging a file by reading it instead would mean hashing every byte of whatever was
 * dropped - a video, an archive, a folder - before concluding it is not a photograph, and would
 * report a corrupt but genuinely supported photograph as a format the application does not open.
 *
 * What is refused is named to the user here rather than in Rust, which is also where the reference
 * puts it and for the reason its comment gives: the notice needs a plural form that only the i18n
 * catalogue gets right in languages with more than two. Refusing a file never costs the user the rest
 * of the drop - a folder of photographs with a stray document in it is a batch they assembled.
 *
 * **It also reports whether a drag is currently over `target`**, which is the second answer the same
 * channel already holds - the returned boolean is the `isOver` call the drop path makes, evaluated
 * while the files are still in the air. A separate hook with its own `onDragDropEvent` would need
 * its own copy of `positionScale` and `isOver`, and two copies are how the region that is drawn and
 * the region that accepts a drop come to disagree. Opening dropped images is still what this hook is
 * for; the hover is a second return from one listener, not a second responsibility.
 */
export const useDroppedImages = (target: RefObject<HTMLElement | null>) => {
    const { t } = useTranslation();
    const addFiles = useFileStore((state) => state.addFiles);
    const [over, setOver] = useState(false);

    useEffect(() => {
        const open = async (paths: string[]) => {
            const extensions = await inputExtensions();

            // One pass rather than two complementary filters: `extensionOf` allocates and the
            // `includes` is linear over the whole list, so a folder drop of several hundred paths
            // would evaluate both twice for every one of them to reach the same two arrays.
            const supported: string[] = [];
            const refused: string[] = [];

            for (const path of paths) {
                (extensions.includes(extensionOf(path)) ? supported : refused).push(path);
            }

            // Ahead of the describe, as the reference emits it ahead of the load: the notice is about
            // what was dropped, and it should not wait on however long hashing the rest takes.
            if (refused.length > 0) {
                toast.warning(
                    t("toasts.unsupportedFiles", { count: refused.length, names: refused.map(fileName).join(", ") }),
                );
                // Which formats people want that this application does not open. Their types only.
                track("files_refused", { count: refused.length, formats: formatsOf(refused) });
            }

            // Not "describe nothing and add nothing", which would be the same answer at the cost of a
            // round trip: a drop of one document is a drop this application has nothing to open.
            if (supported.length === 0) return;

            const admitted = await describeImages(supported);
            addFiles(admitted);

            if (admitted.length > 0) {
                track("files_added", {
                    count: admitted.length,
                    source: "drop",
                    formats: formatsOf(admitted.map((file) => file.path)),
                });
            }
        };

        const listening = getCurrentWebview().onDragDropEvent((event) => {
            const payload = event.payload;
            const element = target.current;

            // `leave` carries no position - it is the drag crossing back out of the window - so it
            // clears rather than being hit-tested.
            if (payload.type === "leave") {
                setOver(false);
                return;
            }

            // Recomputed from the position on both, never keyed on `enter` alone: `enter` is a
            // *window* event, fired once wherever the drag crossed in. A drag that arrives over the
            // navbar and slides down onto the canvas emits one `enter` outside it and then only
            // `over`, so an `enter`-only mark never lights up for it - and a mark that lit up on
            // `enter` regardless of position would light up for a drag that never reaches the
            // canvas. Recomputing makes both edges the same rule rather than two special cases.
            if (payload.type === "enter" || payload.type === "over") {
                setOver(element !== null && isOver(element, payload.position));
                return;
            }

            // Unconditionally, and before the hit-test below: the mark goes out when the files are
            // let go of, whether or not this window was the one that took them.
            setOver(false);

            if (payload.type !== "drop") return;

            // Before the extension list is even asked for: a drop outside the canvas is not this
            // window's to open, and nothing is told to the user about it - the file was let go of
            // over a region that never offered to take it.
            if (!element || !isOver(element, payload.position)) return;

            open(payload.paths).catch((error: unknown) =>
                // Either the extension list or the describe, both of which mean the IPC bridge is
                // gone. Rust has written whatever it knows to the log, and `report` records what it
                // could not know.
                report("Failed to open the dropped files", error),
            );
        });

        return () => {
            // The listener is registered asynchronously, so the unregistration has to wait for it -
            // otherwise a component unmounted before the promise settles leaves one behind, and a
            // reloaded webview in development would accumulate a handler per reload.
            listening.then((unlisten) => unlisten());
        };
    }, [target, addFiles, t]);

    return over;
};
