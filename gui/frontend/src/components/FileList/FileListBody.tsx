import { memo } from 'react';
import { FileListItem } from '@/components/FileList/FileListItem.tsx';
import { useFileStore } from '@/stores';

type FileListBodyProps = {
    drawerHeight: number;
};

export const FileListBody = memo(({ drawerHeight }: FileListBodyProps) => {
    const files = useFileStore((state) => state.files);
    const selectedIndex = useFileStore((state) => state.selectedIndex);
    const setSelectedIndex = useFileStore((state) => state.setSelectedIndex);

    const onImageClicked = (index: number) => {
        setSelectedIndex(index);
    };

    return (
        <div style={{ height: drawerHeight }} className='flex flex-row px-4 py-3 gap-4 overflow-x-auto scrollbar-thin'>
            {files.map((file, index) => (
                <FileListItem
                    key={`img-${index}`}
                    index={index}
                    file={file}
                    selected={index === selectedIndex}
                    onClick={() => onImageClicked(index)}
                />
            ))}
        </div>
    );
});
