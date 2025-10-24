import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { FileListButton } from '@/components/FileList/FileListButton.tsx';
import { FileListZoom } from '@/components/FileList/FileListZoom.tsx';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const FileListHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto pl-2 pr-4 bg-[#272727] ${className}`}
        >
            <FileListButton />

            <div className='flex-1' />

            <FileListZoom className='w-44' />
        </div>
    );
};
