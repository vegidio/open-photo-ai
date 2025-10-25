import type { RefObject } from 'react';
import { SwipeableDrawer } from '@mui/material';
import { FileListBody } from '@/components/FileList/FileListBody.tsx';
import { FileListHeader } from '@/components/FileList/FileListHeader.tsx';
import { useFileListStore } from '@/stores';

const drawerBleeding = 48;
const drawerHeight = 128;

type FileListProps = {
    containerRef: RefObject<HTMLDivElement | null>;
};

export const FileList = ({ containerRef }: FileListProps) => {
    const open = useFileListStore((state) => state.open);
    const setOpen = useFileListStore((state) => state.setOpen);

    return (
        <SwipeableDrawer
            id='file_list'
            anchor='bottom'
            open={open}
            onClose={() => setOpen(false)}
            onOpen={() => {}}
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
                backdrop: {
                    invisible: true,
                },
            }}
        >
            <FileListHeader drawerBleeding={drawerBleeding} className='w-full' />

            <FileListBody drawerHeight={drawerHeight} />
        </SwipeableDrawer>
    );
};
