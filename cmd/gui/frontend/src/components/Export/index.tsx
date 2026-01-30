import { Dialog, Divider } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { ExportQueue } from '@/components/Export/ExportQueue.tsx';
import { ExportSettings } from '@/components/Export/ExportSettings.tsx';
import { ModalTitle } from '@/components/molecules/ModalTitle';

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
                    className: 'bg-[#212121] w-[70rem] h-[40rem] max-w-full bg-none',
                },
            }}
        >
            <ModalTitle title='Export' onClose={onClose} />

            <div className='flex flex-row h-full overflow-hidden'>
                <ExportQueue enhancements={enhancements} className='flex-1' />

                <Divider orientation='vertical' flexItem className='border-[#171717] my-0.5' />

                <ExportSettings enhancements={enhancements} onClose={onClose} className='w-80' />
            </div>
        </Dialog>
    );
};
