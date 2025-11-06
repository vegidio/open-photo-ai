import { Divider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { DrawerAddImages } from './DrawerAddImages.tsx';
import { DrawerToggle } from './DrawerToggle';
import { DrawerZoom } from './DrawerZoom';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const DrawerHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto pl-2 pr-4 gap-2 bg-[#272727] ${className}`}
        >
            <DrawerToggle />

            <Divider orientation='vertical' variant='middle' flexItem />

            <DrawerAddImages />

            <Divider orientation='vertical' variant='middle' flexItem />

            <div className='flex-1' />

            <DrawerZoom className='w-44' />
        </div>
    );
};
