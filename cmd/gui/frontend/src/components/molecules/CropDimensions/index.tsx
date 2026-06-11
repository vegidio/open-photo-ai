import { type ChangeEvent, type KeyboardEvent, useEffect, useState } from 'react';
import { Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { IconButton } from '@/components/atoms/IconButton';
import { TextField } from '@/components/atoms/TextField';

type DimensionFieldProps = {
    label: string;
    value: string;
    onCommit: (value: number) => void;
};

const DimensionField = ({ label, value, onCommit }: DimensionFieldProps) => {
    // Local state so partial/empty typing isn't clobbered by the live stencil sync coming from the parent.
    const [input, setInput] = useState(value);
    useEffect(() => setInput(value), [value]);

    const commit = () => {
        const parsed = parseInt(input.trim(), 10);
        if (!Number.isNaN(parsed)) onCommit(parsed);
    };

    return (
        <TextField
            value={input}
            onChange={(e: ChangeEvent<HTMLInputElement>) => setInput(e.target.value)}
            onBlur={commit}
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
