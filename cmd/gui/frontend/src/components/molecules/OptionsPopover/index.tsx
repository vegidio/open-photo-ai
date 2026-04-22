import type { ReactNode } from 'react';
import { ClickAwayListener, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';

type OptionsPopoverProps = {
    title: string;
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
    hideBackdrop?: boolean;
    children: ReactNode;
};

export const OptionsPopover = ({ title, anchorEl, open, onClose, hideBackdrop = true, children }: OptionsPopoverProps) => {
    return (
        <Popover
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={hideBackdrop}
            anchorOrigin={{
                vertical: 'center',
                horizontal: 'left',
            }}
            transformOrigin={{
                vertical: 'top',
                horizontal: 'right',
            }}
            className='pointer-events-none'
            slotProps={{
                paper: {
                    className: 'w-64 -ml-4 pointer-events-auto',
                },
            }}
        >
            <ClickAwayListener onClickAway={onClose}>
                <div className='flex flex-col'>
                    <ModalTitle title={title} onClose={onClose} />

                    {children}
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
