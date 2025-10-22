import type { RefObject } from 'react';
import { Button, Skeleton, SwipeableDrawer } from '@mui/material';
import { Global } from '@emotion/react';
import { grey } from '@mui/material/colors';
import { styled } from '@mui/material/styles';
import { useFileListStore } from '@/stores';

const drawerBleeding = 48;

type FileListProps = {
    containerRef: RefObject<HTMLDivElement | null>;
};

export const FileList = ({ containerRef }: FileListProps) => {
    const open = useFileListStore((state) => state.open);
    const setOpen = useFileListStore((state) => state.setOpen);

    return (
        <>
            <Global
                styles={{
                    '.MuiDrawer-root > .MuiPaper-root': {
                        height: `calc(50% - ${drawerBleeding}px)`,
                        overflow: 'visible',
                        position: 'absolute',
                    },
                }}
            />

            <SwipeableDrawer
                id='file_list'
                anchor='bottom'
                open={open}
                onClose={() => {}}
                onOpen={() => {}}
                disableSwipeToOpen={true}
                keepMounted
                ModalProps={{
                    container: containerRef.current,
                    className: 'absolute',
                }}
            >
                <StyledBox
                    sx={{
                        position: 'absolute',
                        top: -drawerBleeding,
                        visibility: 'visible',
                        right: 0,
                        left: 0,
                        pointerEvents: 'auto',
                    }}
                >
                    <Button
                        type='button'
                        onClick={() => {
                            console.log('toggle');
                            setOpen(!open);
                        }}
                    >
                        Toggle
                    </Button>
                </StyledBox>

                <StyledBox sx={{ px: 2, pb: 2, height: '100%', overflow: 'auto' }}>
                    <Skeleton variant='rectangular' height='100%' />
                </StyledBox>
            </SwipeableDrawer>
        </>
    );
};

const StyledBox = styled('div')(({ theme }) => ({
    backgroundColor: '#fff',
    ...theme.applyStyles('dark', {
        backgroundColor: grey[800],
    }),
}));
