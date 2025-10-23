import type { DialogFile } from '../../../bindings/gui/types';
import { ImagePath } from '@/components/Image';

type FileListItemProps = {
    index: number;
    file: DialogFile;
    selected?: boolean;
    onClick?: () => void;
};

export const FileListItem = ({ index, file, selected = false, onClick }: FileListItemProps) => {
    return (
        <button
            onClick={onClick}
            type='button'
            className={`h-full aspect-square rounded ${selected ? 'outline-3 outline-blue-500' : ''}`}
        >
            <ImagePath file={file} size={100} className='w-full h-full object-cover rounded' />
        </button>
    );
};
