import { Button } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { FileListZoom } from '@/components/FileList/FileListZoom.tsx';
import { useFileListStore } from '@/stores';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const FileListHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    const toggle = useFileListStore((state) => state.toggle);

    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto pl-2 pr-4 bg-[#272727] ${className}`}
        >
            <Button type='button' onClick={toggle}>
                Toggle
            </Button>

            <div className='flex-1' />

            <FileListZoom className='w-44' />
        </div>
    );
};
