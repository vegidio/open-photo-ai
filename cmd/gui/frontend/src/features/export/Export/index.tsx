import { useState } from 'react';
import { Dialog, Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ExportQueue } from '@/features/export/ExportQueue';
import { ExportSettings } from '@/features/export/ExportSettings';

type ExportProps = {
    enhancements: Map<File, Operation[]>;
    open: boolean;
    onClose: () => void;
};

export const Export = ({ enhancements, open, onClose }: ExportProps) => {
    const { t } = useTranslation();

    // Closing while a batch is running unmounts the subtree that owns it. ExportSettingsButtons cancels the batch on
    // unmount so nothing runs on, but silently discarding a half-finished export is not what pressing Escape should
    // mean - Abort is there to say that deliberately. So the two exits that were unconditional, Escape and the title
    // bar's X, are held shut while it runs; backdropClick was already blocked.
    const [busy, setBusy] = useState(false);

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick' && !busy) {
                    onClose();
                }
            }}
            slotProps={{
                paper: {
                    className: 'bg-surface w-[70rem] h-[42rem] max-w-full bg-none',
                },
            }}
        >
            {/* ModalTitle hides the X when it has no handler, which is the signal wanted here: while the batch runs
                the way out is the Abort button, not a close box that would look like it cancelled cleanly. */}
            <ModalTitle title={t('export.title')} onClose={busy ? undefined : onClose} />

            <div className='flex flex-row h-full overflow-hidden'>
                <ExportQueue enhancements={enhancements} className='flex-1' />

                <Divider orientation='vertical' flexItem className='border-surface-base my-0.5' />

                <ExportSettings enhancements={enhancements} onClose={onClose} onBusyChange={setBusy} className='w-80' />
            </div>
        </Dialog>
    );
};
