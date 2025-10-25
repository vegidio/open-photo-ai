import { Divider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { FileListButton } from './FileListButton.tsx';
import { FileListZoom } from './FileListZoom.tsx';
import { FileListAddImages } from '@/components/FileList/FileListAddImages.tsx';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const FileListHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto pl-2 pr-4 gap-2 bg-[#272727] ${className}`}
        >
            <FileListButton />

            <Divider orientation='vertical' variant='middle' flexItem />

            <FileListAddImages />

            <Divider orientation='vertical' variant='middle' flexItem />

            <div className='flex-1' />

            <FileListZoom className='w-44' />
        </div>
    );
};
