import { useEffect, useState } from 'react';
import type { DialogFile } from '../../../bindings/gui/types';
import { getImage } from '@/utils/image.ts';

type FileListItemProps = {
    index: number;
    file: DialogFile;
    selected?: boolean;
    onClick?: () => void;
};

export const FileListItem = ({ index, file, selected = false, onClick }: FileListItemProps) => {
    const [imageUrl, setImageUrl] = useState<string>();

    useEffect(() => {
        async function loadImage() {
            const imageUrl = await getImage(file, 100);
            setImageUrl(imageUrl);
        }

        loadImage();
    }, [file]);

    return (
        <button
            onClick={onClick}
            type='button'
            className={`h-full aspect-square rounded ${selected ? 'outline-3 outline-blue-500' : ''}`}
        >
            <img alt='Preview' src={imageUrl} className='w-full h-full object-cover rounded' />
        </button>
    );
};
