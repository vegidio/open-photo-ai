import type { MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup, Tooltip, Typography } from '@mui/material';
import { Icon } from '@/components/atoms/Icon';

export type ModelSelectorOption = {
    value: string;
    label: string;
    description?: string;
    disabled?: boolean;
};

type ModelSelectorProps = {
    options: ModelSelectorOption[];
    value?: string;
    onChange?: (value: string) => void;
};

export const ModelSelector = ({ options, value, onChange }: ModelSelectorProps) => {
    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: string) => {
        if (newValue) onChange?.(newValue);
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
                {options.map(({ value, label, description, disabled = false }) => (
                    <ToggleButton
                        key={value}
                        size='small'
                        value={value}
                        disabled={disabled}
                        className='relative border-0 rounded'
                    >
                        {description && (
                            <Tooltip
                                title={description}
                                placement='left'
                                slotProps={{ tooltip: { className: 'mr-6.5 !bg-black/70' } }}
                            >
                                <span className='absolute top-1 left-1'>
                                    <Icon option='info' className='size-3.5 text-yellow-300' />
                                </span>
                            </Tooltip>
                        )}

                        <Typography className={`text-[13px] normal-case font-normal ${description ? 'ml-3' : ''}`}>
                            {label}
                        </Typography>
                    </ToggleButton>
                ))}
            </ToggleButtonGroup>
        </div>
    );
};
