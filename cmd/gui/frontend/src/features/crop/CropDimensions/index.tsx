import { type ChangeEvent, type KeyboardEvent, useEffect, useState } from 'react';
import { Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { IconButton } from '@/components/atoms/IconButton';
import { TextField } from '@/components/atoms/TextField';
import { MIN_CROP_SIZE } from '@/utils/constants.ts';

const toInt = (value: string) => Number.parseInt(value.trim(), 10);

type DimensionFieldProps = {
    label: string;
    value: string;
    onCommit: (value: number) => void;
};

const DimensionField = ({ label, value, onCommit }: DimensionFieldProps) => {
    // Local state so partial/empty typing isn't clobbered by the live stencil sync coming from the parent.
    const [input, setInput] = useState(value);
    const [focused, setFocused] = useState(false);

    // Mirror the parent value only while not editing: keeps live commits / drag syncs from overwriting partial
    // typing, and snaps the display back to the clamped value once the field loses focus.
    useEffect(() => {
        if (!focused) setInput(value);
    }, [value, focused]);

    // Commit on every keystroke for an immediate resize; skip non-numbers so a mid-edit empty field doesn't fight
    // the user. The parent clamps to [MIN_CROP_SIZE, imageDimension].
    const onChange = (e: ChangeEvent<HTMLInputElement>) => {
        setInput(e.target.value);
        const parsed = toInt(e.target.value);
        if (!Number.isNaN(parsed)) onCommit(parsed);
    };

    // On blur, an empty/invalid field falls back to the smallest allowed size; clearing focus re-runs the sync
    // effect, which snaps the display to the final clamped value.
    const onBlur = () => {
        if (Number.isNaN(toInt(input))) onCommit(MIN_CROP_SIZE);
        setFocused(false);
    };

    return (
        <TextField
            value={input}
            onFocus={() => setFocused(true)}
            onChange={onChange}
            onBlur={onBlur}
            onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
                if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            }}
            className='flex-1 m-0'
            slotProps={{
                input: {
                    startAdornment: <Typography className='mr-2 text-[#b0b0b0]'>{label}</Typography>,
                },
            }}
        />
    );
};

type CropDimensionsProps = TailwindProps & {
    width: string;
    height: string;
    onWidthCommit: (value: number) => void;
    onHeightCommit: (value: number) => void;
    onSwap: () => void;
};

export const CropDimensions = ({
    width,
    height,
    onWidthCommit,
    onHeightCommit,
    onSwap,
    className,
}: CropDimensionsProps) => {
    return (
        <div className={`flex flex-row items-center gap-2 ${className ?? ''}`}>
            <DimensionField label='w' value={width} onCommit={onWidthCommit} />
            <IconButton option='swap' size='small' onClick={onSwap} className='shrink-0' />
            <DimensionField label='h' value={height} onCommit={onHeightCommit} />
        </div>
    );
};
