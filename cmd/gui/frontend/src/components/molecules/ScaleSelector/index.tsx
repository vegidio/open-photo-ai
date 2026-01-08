import type { ChangeEvent, MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup, Typography } from '@mui/material';
import { Button } from '@/components/atoms/Button';
import { TextField } from '@/components/atoms/TextField';

type ScaleSelectorProps = {
    value: string;
    onChange?: (value: string) => void;
};

export const ScaleSelector = ({ value, onChange }: ScaleSelectorProps) => {
    const onTextChange = (e: ChangeEvent<HTMLInputElement>) => {
        const inputValue = e.target.value.trim();

        // Don't allow empty values, set 1 instead
        if (inputValue === '') {
            onChange?.('');
            return;
        }

        // The last character is a decimal separator, so we wait for the next digit before converting
        if (inputValue.endsWith('.')) {
            onChange?.(inputValue);
            return;
        }

        const numValue = parseFloat(inputValue);

        // Validate: must be a number between 1-8
        if (!Number.isNaN(numValue)) {
            const clampedValue = Math.max(1, Math.min(8, numValue));
            onChange?.(clampedValue.toString());
        }
    };

    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: string) => {
        if (newValue && newValue !== '-') onChange?.(newValue);
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2'>Scale</Typography>

            <div className='flex flex-row gap-2 items-center'>
                <TextField
                    value={value}
                    onChange={onTextChange}
                    className='flex-3 m-0'
                    slotProps={{
                        input: {
                            endAdornment: <Typography>x</Typography>,
                        },
                    }}
                />

                <Button option='secondary' size='small' className='h-8.5 flex-1' onClick={() => onChange?.('8')}>
                    Max
                </Button>
            </div>

            <ToggleButtonGroup value={value} exclusive onChange={onButtonClick} className='bg-[#171717] gap-1 p-1 flex'>
                <ToggleButton size='small' value='1' className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>1x</Typography>
                </ToggleButton>
                <ToggleButton size='small' value='2' className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>2x</Typography>
                </ToggleButton>
                <ToggleButton size='small' value='4' className='flex-1 border-0 rounded'>
                    <Typography className='text-[13px] normal-case font-normal'>4x</Typography>
                </ToggleButton>
                <ToggleButton
                    size='small'
                    value={['1', '2', '4'].includes(value) ? '-' : value}
                    className='flex-2 border-0 rounded'
                >
                    <Typography className='text-[13px] normal-case font-normal'>Custom</Typography>
                </ToggleButton>
            </ToggleButtonGroup>
        </div>
    );
};
