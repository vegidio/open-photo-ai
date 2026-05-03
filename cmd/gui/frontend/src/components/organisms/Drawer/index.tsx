import type { RefObject } from 'react';
import { ClickAwayListener } from '@mui/material';
import { DrawerBody } from '@/components/organisms/DrawerBody';
import { DrawerHeader } from '@/components/organisms/DrawerHeader';
import { useDrawerStore } from '@/stores/drawer.ts';

const drawerBleeding = 48;
const drawerHeight = 128;

type DrawerProps = {
    containerRef: RefObject<HTMLDivElement | null>;
};

export const Drawer = (_props: DrawerProps) => {
    const open = useDrawerStore((state) => state.open);
    const setOpen = useDrawerStore((state) => state.setOpen);

    return (
        <ClickAwayListener onClickAway={() => open && setOpen(false)}>
            <div
                id='file_list'
                className='absolute inset-x-0 bottom-0 z-10 bg-[#272727] text-white transition-transform duration-300 ease-out'
                style={{
                    height: drawerHeight + drawerBleeding,
                    transform: open ? 'translateY(0)' : `translateY(${drawerHeight}px)`,
                }}
            >
                <DrawerHeader drawerBleeding={drawerBleeding} className='w-full' />
                <DrawerBody drawerHeight={drawerHeight} />
            </div>
        </ClickAwayListener>
    );
};
