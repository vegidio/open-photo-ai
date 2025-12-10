import { Dialog, DialogTitle, Divider, IconButton } from '@mui/material';
import { MdClose } from 'react-icons/md';
import { ExportQueue } from '@/components/Export/ExportQueue.tsx';
import { ExportSettings } from '@/components/Export/ExportSettings.tsx';

type ExportProps = {
    open: boolean;
    onClose: () => void;
};

export const Export = ({ open, onClose }: ExportProps) => {
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
                    className: 'bg-[#212121] w-5xl h-[40rem] max-w-full',
                    sx: {
                        backgroundImage: 'none',
                    },
                },
            }}
        >
            <DialogTitle className='p-3 text-xs text-[#9e9e9e]'>Export</DialogTitle>

            <IconButton
                aria-label='close'
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

            <div className='h-full flex flex-row'>
                <ExportQueue className='flex-1' />
                <ExportSettings onClose={onClose} className='w-80' />
            </div>
        </Dialog>
    );
};
