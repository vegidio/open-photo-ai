import { useCallback, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { DrawerItem } from "@/features/drawer/DrawerItem";
import { useResizeObserver } from "@/hooks/useResizeObserver";
import { useFileStore } from "@/stores/files";

/** The gap between items, matching the `gap-4` on the strip. */
const GAP = 16;

/** The strip's vertical padding, matching `py-3`. */
const PADDING_Y = 12;

// Enough that a fast drag does not expose a gap, few enough that the mount cost windowing exists to
// avoid does not creep back in. The reference's number.
/** How many items beyond the visible range stay mounted. */
const OVERSCAN = 4;

type DrawerBodyProps = {
    /** The strip's height, which the drawer owns and this measures its item size out of. */
    height: number;
    // Not the strip, although it is nearer: the strip clips horizontally, so a menu rising above it
    // would be cut off. The drawer's root sets no `overflow-hidden`, which is what lets the menu be
    // drawn above its top edge, and it is the element the fold-on-click-away check tests containment
    // against - see `Drawer`.
    /** The drawer's own element, passed straight through to each thumbnail's file options menu. */
    drawer?: HTMLElement | null | undefined;
};

/**
 * The strip of open images: one thumbnail per file, in the order they were opened. Windowed: only the
 * items in view, and `OVERSCAN` either side, are mounted.
 */
export const DrawerBody = ({ height, drawer }: DrawerBodyProps) => {
    // Windowed rather than mounting every file. Each item mounts a button, an `<img>`, a Radix checkbox
    // and a span, so 500 files would be some 2,000 nodes in a strip that shows about ten - and the node
    // count is not even the expensive part: every item subscribes to the file store with its own
    // selector, and zustand re-runs every subscriber's selector on every write, so one Select all would
    // run 500 selectors on the click's synchronous path. `loading="lazy"` on each thumbnail bounds which
    // of the mounted items are *fetched*; this bounds how many are mounted at all.
    //
    // Two spacers stand in for the unmounted items, rather than the mounted ones being positioned
    // absolutely. The container keeps its flex, padding and gap and lays the thumbnails out by them,
    // so their size, spacing and padding are not this component's arithmetic to get right. Positioning
    // by hand would mean predicting the height a scrollbar leaves behind, a prediction that is easily a
    // few pixels off. Each spacer loses `GAP` from its width because flex adds a gap between it and the
    // item beside it.
    const files = useFileStore((state) => state.files);
    const currentIndex = useFileStore((state) => state.currentIndex);
    const setCurrentIndex = useFileStore((state) => state.setCurrentIndex);

    const scrollRef = useRef<HTMLDivElement>(null);

    /*
     * How wide one item is, which for a square item is the height left inside the strip.
     *
     * Measured rather than derived from `height`, because a visible horizontal scrollbar takes
     * its space out of the content box too - and does so on Windows and GTK while taking none on
     * macOS, where it overlays. Writing that difference into a number here would be a platform
     * assumption trusted rather than read.
     *
     * Being slightly wrong is survivable by construction, which is the point of the spacer layout:
     * this number sizes the two spacers and nothing else, so an error shifts how far the strip
     * scrolls, never how the thumbnails are spaced or padded.
     */
    const [itemSize, setItemSize] = useState(() => Math.max(0, height - 2 * PADDING_Y));

    useResizeObserver(
        useCallback(() => scrollRef.current, []),
        useCallback((element: Element) => setItemSize(Math.max(0, element.clientHeight - 2 * PADDING_Y)), []),
    );

    const virtualizer = useVirtualizer({
        horizontal: true,
        count: files.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => itemSize + GAP,
        overscan: OVERSCAN,
    });

    /*
     * One stable handler for the whole strip: an inline closure per item would give every memoized
     * `DrawerItem` a new prop on each render, defeating the memo and re-rendering every mounted
     * thumbnail on every navigation click.
     */
    const onItemClick = useCallback((index: number) => setCurrentIndex(index), [setCurrentIndex]);

    const virtualItems = virtualizer.getVirtualItems();
    const first = virtualItems[0];
    const last = virtualItems[virtualItems.length - 1];

    const leadWidth = first ? Math.max(0, first.start - GAP) : 0;
    const tailWidth = last ? Math.max(0, virtualizer.getTotalSize() - last.end - GAP) : 0;

    return (
        <div
            ref={scrollRef}
            style={{ height }}
            /*
             * `scrollbar-thin` is `scrollbar-width: thin`, and it is the strip's own metric rather
             * than a preference: the bar the platform draws at its default width eats visibly into a
             * 104px square, and the measurement above reads `clientHeight` precisely so the items
             * shrink to whatever it leaves. Thin keeps that subtraction small on the platforms that
             * inset a scrollbar at all, and costs nothing on macOS, where it overlays.
             */
            className="flex flex-row items-stretch gap-4 overflow-x-auto overflow-y-hidden scrollbar-thin bg-secondary px-4 py-3"
            data-slot="drawer-strip"
        >
            {leadWidth > 0 && <div style={{ width: leadWidth }} className="flex-none" data-slot="drawer-spacer" />}

            {virtualItems.map((virtualItem) => {
                const file = files[virtualItem.index];

                // Unreachable - the virtualizer's count is `files.length` - but `noUncheckedIndexedAccess`
                // types an index read as possibly undefined, and narrowing it here is cheaper than
                // asserting it away.
                if (!file) return null;

                return (
                    // Keyed by path rather than by identity, for the reason `FileStore.selectedPaths` is.
                    <DrawerItem
                        key={file.path}
                        file={file}
                        index={virtualItem.index}
                        current={virtualItem.index === currentIndex}
                        onClick={onItemClick}
                        drawer={drawer}
                    />
                );
            })}

            {tailWidth > 0 && <div style={{ width: tailWidth }} className="flex-none" data-slot="drawer-spacer" />}
        </div>
    );
};
