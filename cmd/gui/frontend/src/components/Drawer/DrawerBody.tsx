import { DrawerItem } from './DrawerItem';
import { useFileStore } from '@/stores';

type FileListBodyProps = {
    drawerHeight: number;
};

export const DrawerBody = ({ drawerHeight }: FileListBodyProps) => {
    const files = useFileStore((state) => state.files);
    const currentIndex = useFileStore((state) => state.currentIndex);
    const setCurrentIndex = useFileStore((state) => state.setCurrentIndex);

    const onImageClicked = (index: number) => {
        setCurrentIndex(index);
    };

    return (
        <div style={{ height: drawerHeight }} className='flex flex-row px-4 py-3 gap-4 overflow-x-auto scrollbar-thin'>
            {files.map((file, index) => (
                <DrawerItem
                    key={`img-${index}`}
                    file={file}
                    current={index === currentIndex}
                    onClick={() => onImageClicked(index)}
                />
            ))}
        </div>
    );
};
