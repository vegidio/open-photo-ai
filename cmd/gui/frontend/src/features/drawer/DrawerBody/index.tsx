import { useCallback, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DrawerItem } from '@/features/drawer/DrawerItem';
import { useResizeObserver } from '@/hooks';
import { useFileStore } from '@/stores';

type FileListBodyProps = {
    drawerHeight: number;
};

// The gap between items, matching the `gap-4` on the strip. It is the one metric this file needs as a number as well
// as a class, because the spacer widths below have to account for the gap flex puts on either side of them.
const GAP = 16;

// The strip's vertical padding, matching `py-3`. Only used to estimate how wide a square item is; the actual layout
// comes from the class.
const PADDING_Y = 12;

// How many items beyond the visible range stay mounted. Enough that a fast drag does not expose a gap, few enough that
// the mount cost this exists to avoid does not creep back in.
const OVERSCAN = 4;

export const DrawerBody = ({ drawerHeight }: FileListBodyProps) => {
    const files = useFileStore((state) => state.files);
    const currentIndex = useFileStore((state) => state.currentIndex);
    const setCurrentIndex = useFileStore((state) => state.setCurrentIndex);

    const scrollRef = useRef<HTMLDivElement>(null);

    // How wide one item is, which for a square item is the height left inside the strip. Measured rather than derived
    // from drawerHeight, because a visible horizontal scrollbar takes its space out of the content box too.
    //
    // Being slightly wrong here is survivable, which is the point of the layout below: this number only sizes the two
    // spacers, so an error shifts how far the strip scrolls, never how the thumbnails are spaced or padded.
    const [itemSize, setItemSize] = useState(() => Math.max(0, drawerHeight - 2 * PADDING_Y));

    useResizeObserver(
        useCallback(() => scrollRef.current, []),
        useCallback((el: Element) => setItemSize(Math.max(0, el.clientHeight - 2 * PADDING_Y)), []),
    );

    // Windowed rather than rendering every file. Each DrawerItem mounts a Checkbox, an IconButton, a Typography and a
    // Menu, so a 500-file drop - the size the stores and useThumbnail were explicitly tuned for - put roughly 2,500 MUI
    // components in a strip that shows about ten. useThumbnail's IntersectionObserver already deferred the decode; this
    // is the other half, the mount itself.
    const virtualizer = useVirtualizer({
        horizontal: true,
        count: files.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => itemSize + GAP,
        overscan: OVERSCAN,
    });

    // One stable handler for the whole strip: an inline closure per item would give every memoized DrawerItem a new
    // prop on each render, defeating the memo and re-rendering every thumbnail on every navigation click.
    const onImageClicked = useCallback((index: number) => setCurrentIndex(index), [setCurrentIndex]);

    const virtualItems = virtualizer.getVirtualItems();
    const first = virtualItems[0];
    const last = virtualItems[virtualItems.length - 1];

    // Two spacers standing in for the items that are not mounted, rather than absolute positioning for the ones that
    // are.
    //
    // The container keeps the exact flex/padding/gap it always had, and the thumbnails are laid out by it exactly as
    // before - so their size, their spacing and the padding around them are not this component's arithmetic to get
    // right. Positioning them by hand meant predicting the height a scrollbar leaves behind, and every version of that
    // prediction was off by a few pixels somewhere.
    //
    // Each spacer loses GAP from its width because flex adds a gap between it and the item beside it.
    const leadWidth = first ? Math.max(0, first.start - GAP) : 0;
    const tailWidth = last ? Math.max(0, virtualizer.getTotalSize() - last.end - GAP) : 0;

    return (
        <div
            ref={scrollRef}
            style={{ height: drawerHeight }}
            className='flex flex-row px-4 py-3 gap-4 overflow-x-auto scrollbar-thin bg-surface-overlay'
        >
            {leadWidth > 0 && <div style={{ width: leadWidth }} className='shrink-0' />}

            {virtualItems.map((virtualItem) => {
                const file = files[virtualItem.index];

                // Unreachable - the virtualizer's count is files.length - but noUncheckedIndexedAccess types an index
                // read as possibly undefined, and narrowing it here is cheaper than asserting it away.
                if (!file) return null;

                return (
                    <DrawerItem
                        key={file.Hash}
                        file={file}
                        index={virtualItem.index}
                        current={virtualItem.index === currentIndex}
                        onClick={onImageClicked}
                    />
                );
            })}

            {tailWidth > 0 && <div style={{ width: tailWidth }} className='shrink-0' />}
        </div>
    );
};
