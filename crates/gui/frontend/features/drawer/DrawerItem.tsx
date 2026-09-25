import { memo } from "react";
import { useTranslation } from "react-i18next";
import { Checkbox } from "@/components/ui/checkbox";
import { FileMenuTrigger, FileOptionsMenu, THUMBNAIL_ANCHOR } from "@/features/files/FileOptionsMenu";
import type { ImageRecord } from "@/ipc/images";
import { renditionUrl } from "@/ipc/images";
import { THUMBNAIL_BOUND } from "@/lib/constants";
import { fileName } from "@/lib/paths";
import { cn } from "@/lib/utils";
import { useFileStore } from "@/stores/files";

type DrawerItemProps = {
    /** The photograph this thumbnail stands for. */
    file: ImageRecord;
    /** Where it sits in the strip, handed back through {@link onClick}. */
    index: number;
    /** Whether this is the image the window is drawing. */
    current: boolean;
    /** Makes this image the current one. One stable handler for the whole strip - see `DrawerBody`. */
    onClick: (index: number) => void;
    /**
     * The drawer's own element, which this item's menu is portalled into - see `Drawer`. `null`
     * before the drawer's ref has attached, which is one render.
     */
    drawer?: HTMLElement | null | undefined;
};

/**
 * One open image in the strip: the photograph, a checkbox, and a bar carrying its name and the
 * file options trigger.
 *
 * A file whose pixels cannot be served draws the surface behind the image instead, with the name bar
 * and the checkbox intact.
 */
const DrawerItemComponent = ({ file, index, current, onClick, drawer }: DrawerItemProps) => {
    // One button, with the checkbox beside it rather than inside it: the item is a positioned wrapper
    // holding a `<button>` covering the square, the checkbox overlaid on its top right corner, and the
    // name bar. The reference makes the item a `<div>` with an `onClick` to avoid nested buttons, and
    // suppresses two accessibility lint rules to do it. Siblings avoid the nesting without avoiding the
    // semantics: the item is a real button, tabbable and operable by space and enter, the checkbox is a
    // real checkbox rather than a control inside a control, and the name bar's three-dot control can be
    // a real trigger - a button inside a button is invalid markup.
    const { t } = useTranslation();

    // Each item subscribes with its own membership test rather than reading the whole Set: one Select
    // all re-runs a boolean selector per mounted item and re-renders only the ones whose answer changed.
    const selected = useFileStore((state) => state.selectedPaths.has(file.path));
    const toggleSelected = useFileStore((state) => state.toggleSelected);

    const name = fileName(file.path);

    return (
        <div
            className={cn(
                "relative aspect-square h-full flex-none rounded-lg",
                // 3px, which is the design's own outline width and not one of Tailwind's steps. Drawn
                // as an outline rather than a border or a ring so it costs the thumbnail no space:
                // a border would shrink the image inside a square whose side the virtualizer has
                // already been told.
                current && "outline-3 outline-primary",
            )}
            data-slot="drawer-item"
        >
            <button
                type="button"
                onClick={() => onClick(index)}
                // Named explicitly: the name bar is a sibling of the button rather than inside it, so it
                // lends the button no name. `DrawerItem.test.tsx` asserts it, because an unnamed button
                // renders exactly like a named one.
                aria-label={name}
                className="absolute inset-0 overflow-hidden rounded-lg bg-surface-thumbnail"
            >
                {/*
                 * `alt=""` rather than `common.previewAlt`, which is the reference's: this element is
                 * inside the button whose name is already the file's, so a label here is the same
                 * thumbnail announced twice, once with the wrong noun.
                 *
                 * `loading="lazy"` is the whole of how a thumbnail's pixels are deferred. The
                 * reference needs a `useThumbnail` hook holding an `IntersectionObserver` because a
                 * thumbnail there is an IPC round trip returning bytes that become an object URL
                 * which must later be revoked; here a thumbnail is a URL, and lazy loading is the
                 * platform's own version of the same deferral on the same trigger.
                 *
                 * Spread conditionally for the reason `PreviewImage`'s pane gives: under
                 * `exactOptionalPropertyTypes` an explicit `undefined` is not an absent prop, and
                 * `src=""` would resolve against the document and fetch the page itself.
                 *
                 * **No framing**, deliberately, where the sidebar's miniature carries one. The strip
                 * answers "which photographs are open", and a thumbnail that changes shape as a crop
                 * is dragged answers it worse. The reference behaves the same way - `useThumbnail.ts`
                 * passes no crop while `Preview/index.tsx` passes one - and it is written down here
                 * because a cropped miniature beside an uncropped strip reads as an oversight
                 * otherwise.
                 */}
                <img
                    {...(file.identity && { src: renditionUrl(file.identity, THUMBNAIL_BOUND) })}
                    alt=""
                    loading="lazy"
                    className="size-full object-cover"
                />
            </button>

            {/*
             * Inverted when this is the current image - a light bar with dark text, against the dark
             * bar with light text every other item draws - which is the second half of how the strip
             * says which photograph the window is showing. Both are translucent over the photograph,
             * as the design draws them.
             *
             * The uncurrent bar draws its name and its glyph at `--foreground` rather than at the dim
             * tier: the bar is translucent over a photograph rather than over a surface, so the tier
             * that reads as a quieter label on a card reads as an unlit one here - and the name is the
             * only thing telling two thumbnails of one photograph apart.
             *
             * `pointer-events-none` on the bar with the trigger exempted: the bar sits over the
             * button's lower edge, and without this it would swallow the presses that choose the
             * image along that strip.
             */}
            <div
                className={cn(
                    "pointer-events-none absolute inset-x-0 bottom-0 flex h-[22px] items-center gap-1 rounded-b-lg px-1.5",
                    current ? "bg-foreground/85 text-background" : "bg-background/75 text-foreground",
                )}
            >
                <span className="min-w-0 flex-1 truncate text-left text-[11px]">{name}</span>

                <FileOptionsMenu file={file} anchor={THUMBNAIL_ANCHOR} container={drawer}>
                    <FileMenuTrigger name={name} className="pointer-events-auto" />
                </FileOptionsMenu>
            </div>

            {/*
             * Named with the file's own name, because a strip of twenty otherwise announces twenty
             * identical checkboxes with nothing to tell them apart. The reference leaves it
             * unlabelled.
             *
             * `z-10` over the button it is a sibling of, and `border-white/70 bg-background/55` while
             * unpicked - the design's values - so it reads against whatever photograph is behind it
             * rather than against a surface. Picked, the vendored checkbox's own
             * `data-[state=checked]:bg-primary` is the accent the design fills it with.
             */}
            <Checkbox
                checked={selected}
                onCheckedChange={() => toggleSelected(file.path)}
                aria-label={t("drawer.selectImage", { name })}
                className="absolute top-1 right-1 z-10 size-[18px] border-white/70 bg-background/55"
            />
        </div>
    );
};

// The strip renders one of these per mounted file and each loads its own thumbnail: without the memo,
// clicking any item re-renders every other.
export const DrawerItem = memo(DrawerItemComponent);
