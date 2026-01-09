import type { MouseEvent } from 'react';
import { ToggleButton, ToggleButtonGroup } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Icon } from '@/components/atoms/Icon';
import { useAppStore } from '@/stores';

type PreviewSelectorProps = TailwindProps & {
    disabled?: boolean;
};

export const PreviewSelector = ({ disabled = false, className = '' }: PreviewSelectorProps) => {
    const previewModel = useAppStore((state) => state.previewMode);
    const setPreviewMode = useAppStore((state) => state.setPreviewMode);

    const onButtonClick = (_: MouseEvent<HTMLElement>, newValue: 'full' | 'side' | 'split') => {
        if (newValue) setPreviewMode(newValue);
    };

    return (
        <ToggleButtonGroup
            value={disabled ? undefined : previewModel}
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
