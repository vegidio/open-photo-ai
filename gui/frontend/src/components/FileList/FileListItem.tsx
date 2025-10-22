import { ImagePath } from '@/components/Image';

type FileListItemProps = {
    index: number;
    path: string;
    onClick?: () => void;
};

export const FileListItem = ({ index, path, onClick }: FileListItemProps) => {
    return (
        <button onClick={onClick} type='button' className='h-full aspect-square'>
            <ImagePath path={path} size={100} className='w-full h-full object-cover' />
        </button>
    );
};
