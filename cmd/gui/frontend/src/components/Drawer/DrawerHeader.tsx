import { Divider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { DrawerAddImages } from './DrawerAddImages.tsx';
import { DrawerSelectAll } from './DrawerSelectAll.tsx';
import { DrawerToggle } from './DrawerToggle';
import { DrawerZoom } from './DrawerZoom';
import { useFileStore } from '@/stores';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const DrawerHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    const fileCount = useFileStore((state) => state.files.length);

    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto pl-0.5 pr-4 gap-1 bg-[#272727] ${className}`}
        >
            <DrawerToggle disabled={fileCount === 0} />

            <Divider orientation='vertical' variant='middle' flexItem />

            <DrawerAddImages />

            <Divider orientation='vertical' variant='middle' flexItem />

            <DrawerSelectAll disabled={fileCount === 0} className='ml-0.5' />

            <Divider orientation='vertical' variant='middle' flexItem />

            <div className='flex-1' />

            <DrawerZoom className='w-44' />
        </div>
    );
};
