import type { MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup, Typography } from '@mui/material';

type ScaleSelectorProps = {
    value?: number;
    onChange?: (value: number) => void;
};

export const ScaleSelector = ({ value, onChange }: ScaleSelectorProps) => {
    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: number) => {
        onChange?.(newValue);
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2'>Scale</Typography>

            <ToggleButtonGroup value={value} exclusive onChange={onButtonClick} className='bg-[#171717] gap-1 p-1 flex'>
                <ToggleButton size='small' value={1} className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>1x</Typography>
                </ToggleButton>
                <ToggleButton size='small' value={2} className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>2x</Typography>
                </ToggleButton>
                <ToggleButton size='small' value={4} className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>4x</Typography>
                </ToggleButton>
                <ToggleButton size='small' value={-1} className='flex-2 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>Custom</Typography>
                </ToggleButton>
            </ToggleButtonGroup>
        </div>
    );
};
