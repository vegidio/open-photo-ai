import type { MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Icon } from '@/components/atoms/Icon';

type PreviewSelectorProps = TailwindProps & {
    value: string;
    onChange?: (value: string) => void;
    disabled?: boolean;
};

export const PreviewSelector = ({ value, onChange, disabled = false, className = '' }: PreviewSelectorProps) => {
    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: string) => {
        if (newValue) onChange?.(newValue);
    };

    return (
        <ToggleButtonGroup
            value={disabled ? undefined : value}
            disabled={disabled}
            exclusive
            onChange={onButtonClick}
            className={`${className}`}
        >
            <ToggleButton value='full' className='w-12 border-0 rounded-none'>
                <Icon option='preview_full' className='size-5.5' />
            </ToggleButton>
            <ToggleButton value='side' className='w-12 border-0 rounded-none'>
                <Icon option='preview_side' className='size-5' />
            </ToggleButton>
            <ToggleButton value='split' className='w-12 border-0 rounded-none'>
                <Icon option='preview_split' className='size-4.5' />
            </ToggleButton>
        </ToggleButtonGroup>
    );
};
