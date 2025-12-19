import type { RefObject } from 'react';
import { ClickAwayListener, SwipeableDrawer } from '@mui/material';
import { DrawerBody } from '@/components/Drawer/DrawerBody.tsx';
import { DrawerHeader } from '@/components/Drawer/DrawerHeader.tsx';
import { useDrawerStore } from '@/stores/drawer.ts';

const drawerBleeding = 48;
const drawerHeight = 128;

type FileListProps = {
    containerRef: RefObject<HTMLDivElement | null>;
};

export const Drawer = ({ containerRef }: FileListProps) => {
    const open = useDrawerStore((state) => state.open);
    const setOpen = useDrawerStore((state) => state.setOpen);

    return (
        <SwipeableDrawer
            id='file_list'
            anchor='bottom'
            open={open}
            onClose={() => setOpen(false)}
            onOpen={() => {}}
            disableSwipeToOpen={true}
            keepMounted
            hideBackdrop
            ModalProps={{
                container: containerRef.current,
                className: 'absolute',
            }}
            slotProps={{
                paper: {
                    sx: {
                        height: `${drawerHeight - drawerBleeding}px)`,
                        overflow: 'visible',
                        position: 'absolute',
                    },
                },
            }}
        >
            <ClickAwayListener onClickAway={() => open && setOpen(false)}>
                <div>
                    <DrawerHeader drawerBleeding={drawerBleeding} className='w-full' />
                    <DrawerBody drawerHeight={drawerHeight} />
                </div>
            </ClickAwayListener>
        </SwipeableDrawer>
    );
};
