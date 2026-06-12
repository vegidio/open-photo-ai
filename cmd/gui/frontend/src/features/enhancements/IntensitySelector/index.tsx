import { type ChangeEvent, type MouseEvent, useEffect, useState } from 'react';
import { Slider, Typography } from '@mui/material';
import { TextField } from '@/components/atoms/TextField';

type IntensitySelectorProps = {
    value: string;
    onChange?: (value: string) => void;
    min?: number;
    max?: number;
    step?: number;
    marks?: { value: number; label: string }[];
};

export const IntensitySelector = ({
    value,
    onChange,
    min = -100,
    max = 100,
    step = 5,
    marks = [{ value: 0, label: '0' }],
}: IntensitySelectorProps) => {
    const [sliderValue, setSliderValue] = useState(value === '' || value === '-' ? 0 : parseFloat(value));

    // Keep the slider in sync when the value changes externally (e.g. typed in the textfield).
    useEffect(() => {
        if (value === '' || value === '-') return;
        const parsed = parseFloat(value);
        if (!Number.isNaN(parsed)) setSliderValue(parsed);
    }, [value]);

    const onTextChange = (e: ChangeEvent<HTMLInputElement>) => {
        const inputValue = e.target.value.trim();

        // Don't allow empty values
        if (inputValue === '' || inputValue === '-') {
            onChange?.(inputValue);
            return;
        }

        const numValue = parseInt(inputValue, 10);

        // Validate: must be a number within the [min, max] range
        if (!Number.isNaN(numValue)) {
            const clampedValue = Math.max(min, Math.min(max, numValue));
            onChange?.(clampedValue.toString());
        }
    };

    const onSliderChange = (_: Event, newValue: number) => {
        setSliderValue(newValue);
    };

    const onMouseUp = (_: MouseEvent<HTMLSpanElement>) => {
        onChange?.(sliderValue.toString());
    };

    return (
        <div className='flex flex-col gap-2'>
            <div className='flex flex-row justify-between items-center'>
                <Typography variant='body2'>Intensity</Typography>
                <TextField
                    value={value}
                    onChange={onTextChange}
                    className='w-20 m-0'
                    slotProps={{
                        input: {
                            endAdornment: <Typography>%</Typography>,
                        },
                    }}
                />
            </div>

            <div className='mx-1'>
                <Slider
                    size='small'
                    min={min}
                    max={max}
                    step={step}
                    marks={marks}
                    track={false}
                    valueLabelDisplay='auto'
                    value={sliderValue}
                    onChange={onSliderChange}
                    onMouseUp={onMouseUp}
                    className='block'
                />
            </div>
        </div>
    );
};
