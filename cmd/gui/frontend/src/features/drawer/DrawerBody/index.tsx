import { useCallback } from 'react';
import { DrawerItem } from '@/features/drawer/DrawerItem';
import { useFileStore } from '@/stores';

type FileListBodyProps = {
    drawerHeight: number;
};

export const DrawerBody = ({ drawerHeight }: FileListBodyProps) => {
    const files = useFileStore((state) => state.files);
    const currentIndex = useFileStore((state) => state.currentIndex);
    const setCurrentIndex = useFileStore((state) => state.setCurrentIndex);

    // One stable handler for the whole strip: an inline closure per item would give every memoized DrawerItem a new
    // prop on each render, defeating the memo and re-rendering every thumbnail on every navigation click.
    const onImageClicked = useCallback((index: number) => setCurrentIndex(index), [setCurrentIndex]);

    return (
        <div
            style={{ height: drawerHeight }}
            className='flex flex-row px-4 py-3 gap-4 overflow-x-auto scrollbar-thin bg-[#353535]'
        >
            {files.map((file, index) => (
                <DrawerItem
                    key={file.Hash}
                    file={file}
                    index={index}
                    current={index === currentIndex}
                    onClick={onImageClicked}
                />
            ))}
        </div>
    );
};
