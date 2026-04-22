import { type MouseEvent, useState } from 'react';
import { Divider, Typography } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Button } from '@/components/atoms/Button';
import { DimensionsPopover } from '@/components/molecules/DimensionsPopover';
import { useEnhancementStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

type NavbarDimensionsProps = TailwindProps & {
    file: File;
};

export const NavbarDimensions = ({ file, className = '' }: NavbarDimensionsProps) => {
    const operations = useEnhancementStore((state) =>
        file ? (state.enhancements.get(file) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );
    const scaleStr = operations.find((op) => op.id.startsWith('up'))?.options?.scale ?? '1';
    const scale = parseFloat(scaleStr);

    const originalDims = `${file.Dimensions[0]} x ${file.Dimensions[1]}`;
    const outputDims = `${(file.Dimensions[0] * scale).toFixed(0)} x ${(file.Dimensions[1] * scale).toFixed(0)}`;

    const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
    const open = Boolean(anchorEl);

    const onPopoverOpen = (event: MouseEvent<HTMLButtonElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onPopoverClose = () => {
        setAnchorEl(null);
    };

    return (
        <>
            <div className={`${className} flex flex-row h-full items-center gap-4`}>
                <Button
                    option='text'
                    size='small'
                    onMouseEnter={onPopoverOpen}
                    onMouseLeave={onPopoverClose}
                    className={`flex flex-col items-center px-3 py-0.5`}
                >
                    <Typography variant='caption' className='text-[#f2f2f2]'>
                        Dimensions
                    </Typography>

                    <Typography variant='caption' className='text-[#b0b0b0]'>
                        {scale > 1 ? outputDims : originalDims}
                    </Typography>
                </Button>

                <Divider orientation='vertical' variant='middle' flexItem />
            </div>

            {open && (
                <DimensionsPopover
                    originalDims={originalDims}
                    outputDims={outputDims}
                    anchorEl={anchorEl}
                    open={true}
                    onClose={onPopoverClose}
                />
            )}
        </>
    );
};
