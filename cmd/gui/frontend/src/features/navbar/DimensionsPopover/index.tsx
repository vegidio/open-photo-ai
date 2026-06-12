import { ClickAwayListener, Popover, Typography } from '@mui/material';
import { Icon } from '@/components/atoms/Icon';

type DimensionsPopoverProps = {
    originalDims: string;
    outputDims: string;
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

export const DimensionsPopover = ({ originalDims, outputDims, anchorEl, open, onClose }: DimensionsPopoverProps) => {
    return (
        <Popover
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={true}
            anchorOrigin={{
                vertical: 'bottom',
                horizontal: 'center',
            }}
            transformOrigin={{
                vertical: 'top',
                horizontal: 'right',
            }}
            className='pointer-events-none'
            slotProps={{
                paper: {
                    className: 'mt-4',
                },
            }}
        >
            <ClickAwayListener onClickAway={onClose}>
                <div className='flex flex-col gap-2 p-4 pr-5 bg-black border-1 border-[#2b2b2b] rounded'>
                    <div className='flex flex-row items-center gap-3'>
                        <Icon option='upscale' className='size-4' />

                        <Typography variant='body2' className='text-[#f2f2f2]'>
                            Dimensions
                        </Typography>
                    </div>

                    <div className='grid grid-cols-[56px_auto] ml-7 gap-2'>
                        <Typography variant='caption' className='text-[#f2f2f2]'>
                            Original
                        </Typography>

                        <Typography variant='caption' className='text-[#b0b0b0]'>
                            {originalDims}
                        </Typography>

                        <Typography variant='caption' className='text-[#f2f2f2]'>
                            Output
                        </Typography>

                        <Typography variant='caption' className='text-[#b0b0b0]'>
                            {outputDims}
                        </Typography>
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
