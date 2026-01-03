import type { MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup, Typography } from '@mui/material';

export type ModelSelectorOption = {
    value: string;
    label: string;
};

type ModelSelectorProps = {
    options: ModelSelectorOption[];
    value?: string;
    onChange?: (value: string) => void;
};

export const ModelSelector = ({ options, value, onChange }: ModelSelectorProps) => {
    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: string) => {
        onChange?.(newValue);
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2'>AI Model</Typography>

            <ToggleButtonGroup
                value={value}
                exclusive
                onChange={onButtonClick}
                className='bg-[#171717] grid grid-cols-2 gap-1 p-1'
            >
                {options.map(({ value, label }) => (
                    <ToggleButton key={value} size='small' value={value} className='border-0 rounded'>
                        <Typography className='text-[13px] normal-case font-normal'>{label}</Typography>
                    </ToggleButton>
                ))}
            </ToggleButtonGroup>
        </div>
    );
};
