import { Slider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Button } from '@/components/atoms/Button';
import { IconButton } from '@/components/atoms/IconButton';

type RotateControlsProps = TailwindProps & {
    rotation: number;
    onRotationChange: (value: number) => void;
    onRotate90: () => void;
    onFlipHorizontal: () => void;
    onFlipVertical: () => void;
    onReset: () => void;
};

// 13 marks at 15° spacing across the [-90°, 90°] fine-rotation range.
const MARKS = Array.from({ length: 13 }, (_, i) => ({ value: i * 15 - 90 }));

export const RotateControls = ({
    rotation,
    onRotationChange,
    onRotate90,
    onFlipHorizontal,
    onFlipVertical,
    onReset,
    className,
}: RotateControlsProps) => {
    return (
        <div className={`flex flex-row items-center gap-4 px-4 py-2 ${className}`}>
            <IconButton option='rotate' size='small' onClick={onRotate90} className='shrink-0' />

            <IconButton option='flip_horizontal' size='small' onClick={onFlipHorizontal} className='shrink-0' />

            <IconButton option='flip_vertical' size='small' onClick={onFlipVertical} className='shrink-0' />

            <Slider
                size='small'
                min={-90}
                max={90}
                step={1}
                marks={MARKS}
                track={false}
                valueLabelDisplay='auto'
                valueLabelFormat={(value) => `${value}°`}
                value={rotation}
                onChange={(_, value) => onRotationChange(value as number)}
                className='flex-1'
            />

            <Button option='tertiary' className='w-28' onClick={onReset}>
                Reset
            </Button>
        </div>
    );
};
