import type { ReactNode } from 'react';
import { Dialog } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { ModalTitle } from '@/components/molecules/ModalTitle';

export type DialogGeneralProps = TailwindProps & {
    title: string;
    open: boolean;
    onClose?: () => void;
    children?: ReactNode;
};

export const DialogGeneral = ({ title, open, onClose, children, className = '' }: DialogGeneralProps) => {
    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') {
                    onClose?.();
                }
            }}
            slotProps={{
                paper: {
                    className: `${className} bg-[#212121] max-w-full bg-none`,
                },
            }}
        >
            <ModalTitle title={title} onClose={onClose} />

            {children}
        </Dialog>
    );
};
