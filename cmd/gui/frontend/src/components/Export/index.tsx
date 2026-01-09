import { Dialog, DialogTitle, Divider, IconButton } from '@mui/material';
import { MdClose } from 'react-icons/md';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { ExportQueue } from '@/components/Export/ExportQueue.tsx';
import { ExportSettings } from '@/components/Export/ExportSettings.tsx';

type ExportProps = {
    enhancements: Map<File, Operation[]>;
    open: boolean;
    onClose: () => void;
};

export const Export = ({ enhancements, open, onClose }: ExportProps) => {
    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') {
                    onClose();
                }
            }}
            slotProps={{
                paper: {
                    className: 'bg-[#212121] w-[70rem] h-[40rem] max-w-full',
                    sx: {
                        backgroundImage: 'none',
                    },
                },
            }}
        >
            <DialogTitle className='p-3 text-xs text-[#9e9e9e]'>Export</DialogTitle>

            <IconButton
                onClick={onClose}
                sx={(theme) => ({
                    position: 'absolute',
                    right: 4,
                    top: 2,
                    color: theme.palette.grey[500],
                })}
            >
                <MdClose className='size-5' />
            </IconButton>

            <Divider />

            <div className='flex flex-row h-full overflow-hidden'>
                <ExportQueue enhancements={enhancements} className='flex-1' />

                <Divider orientation='vertical' flexItem className='border-[#171717] my-0.5' />

                <ExportSettings enhancements={enhancements} onClose={onClose} className='w-80' />
            </div>
        </Dialog>
    );
};
