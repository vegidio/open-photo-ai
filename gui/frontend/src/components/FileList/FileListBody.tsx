import { memo } from 'react';
import { FileListItem } from '@/components/FileList/FileListItem.tsx';
import { useFileStore } from '@/stores';

type FileListBodyProps = {
    drawerHeight: number;
};

export const FileListBody = memo(({ drawerHeight }: FileListBodyProps) => {
    const filePaths = useFileStore((state) => state.filePaths);
    const setSelectedIndex = useFileStore((state) => state.setSelectedIndex);

    const onImageClicked = (index: number) => {
        setSelectedIndex(index);
    };

    return (
        <div style={{ height: drawerHeight }} className='flex flex-row p-4 gap-4 overflow-x-auto scrollbar-thin'>
            {filePaths.map((path, index) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: N/A
                <FileListItem key={`img-${index}`} index={index} path={path} onClick={() => onImageClicked(index)} />
            ))}
        </div>
    );
});
