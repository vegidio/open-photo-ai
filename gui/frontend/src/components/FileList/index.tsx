import type { RefObject } from 'react';
import { Button, Skeleton, SwipeableDrawer } from '@mui/material';
import type { TailwindProps } from '@/utils';
import { useFileListStore } from '@/stores';

const drawerBleeding = 48;
const drawerHeight = 128;

type FileListProps = {
    containerRef: RefObject<HTMLDivElement | null>;
};

export const FileList = ({ containerRef }: FileListProps) => {
    const open = useFileListStore((state) => state.open);

    return (
        <SwipeableDrawer
            id='file_list'
            anchor='bottom'
            open={open}
            onClose={() => {}}
            onOpen={() => {}}
            hideBackdrop={true}
            disableSwipeToOpen={true}
            keepMounted
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
            <DrawerHeader className='w-full' />

            <DrawerBody />
        </SwipeableDrawer>
    );
};

const DrawerHeader = ({ className = '' }: TailwindProps) => {
    const toggle = useFileListStore((state) => state.toggle);

    return (
        <div
            style={{ height: drawerBleeding, top: -drawerBleeding }}
            className={`flex items-center absolute visible pointer-events-auto bg-[#272727] ${className}`}
        >
            <Button type='button' onClick={toggle}>
                Toggle
            </Button>
        </div>
    );
};

const DrawerBody = () => {
    return (
        <div style={{ height: drawerHeight }}>
            <Skeleton variant='rectangular' height='100%' />
        </div>
    );
};
