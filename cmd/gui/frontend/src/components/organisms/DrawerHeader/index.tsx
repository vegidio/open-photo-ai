import { Divider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { DrawerAddImages } from '@/components/organisms/DrawerAddImages';
import { DrawerSelectAll } from '@/components/organisms/DrawerSelectAll';
import { DrawerToggle } from '@/components/organisms/DrawerToggle';
import { DrawerZoom } from '@/components/organisms/DrawerZoom';
import { PreviewSelector } from '@/components/organisms/PreviewSelector';
import { useFileStore } from '@/stores';

type FileListHeaderProps = TailwindProps & {
    drawerBleeding: number;
};

export const DrawerHeader = ({ drawerBleeding, className = '' }: FileListHeaderProps) => {
    const fileCount = useFileStore((state) => state.files.length);

    return (
        <div
            style={{ height: drawerBleeding }}
            className={`flex items-center pl-0.5 pr-3 gap-1 bg-[#272727] ${className}`}
        >
            <DrawerToggle disabled={fileCount === 0} />

            <Divider orientation='vertical' variant='middle' flexItem />

            <DrawerAddImages />

            <Divider orientation='vertical' variant='middle' flexItem />

            <DrawerSelectAll disabled={fileCount === 0} className='ml-0.5' />

            <div id='spacer' className='flex-1' />

            <PreviewSelector disabled={fileCount === 0} className='h-full' />

            <Divider orientation='vertical' variant='middle' flexItem className='mx-1.5' />

            <DrawerZoom disabled={fileCount === 0} className='w-44' />
        </div>
    );
};
