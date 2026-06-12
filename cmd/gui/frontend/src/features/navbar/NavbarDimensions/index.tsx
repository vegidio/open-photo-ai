import { type MouseEvent, useState } from 'react';
import { Divider, Typography } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Button } from '@/components/atoms/Button';
import { DimensionsPopover } from '@/features/navbar/DimensionsPopover';
import { useFileCrop, useFileOperations } from '@/hooks';
import { cropDimensions } from '@/utils/image.ts';

type NavbarDimensionsProps = TailwindProps & {
    file: File;
};

export const NavbarDimensions = ({ file, className = '' }: NavbarDimensionsProps) => {
    const operations = useFileOperations(file);
    const scaleStr = operations.find((op) => op.id.startsWith('up'))?.options?.scale ?? '1';
    const scale = parseFloat(scaleStr);

    // A crop changes the source dimensions; the crop box (post-rotation) is the cropped image's size.
    const crop = useFileCrop(file);
    const [width, height] = cropDimensions(file, crop);

    const originalDims = `${width} x ${height}`;
    const outputDims = `${(width * scale).toFixed(0)} x ${(height * scale).toFixed(0)}`;

    const [anchorEl, setAnchorEl] = useState<HTMLElement | undefined>(undefined);
    const open = Boolean(anchorEl);

    const onPopoverOpen = (event: MouseEvent<HTMLButtonElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onPopoverClose = () => {
        setAnchorEl(undefined);
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
