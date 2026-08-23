import { type ChangeEvent, useEffect, useState } from 'react';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { TextField } from '@/components/atoms/TextField';
import { ValueSlider } from '@/components/atoms/ValueSlider';

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
    const { t } = useTranslation();
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

    // The slider half is ValueSlider's; only the string-typed TextField contract below is this component's own, so
    // the conversion happens at that boundary rather than by keeping a second copy of the MUI Slider.
    const onSliderChange = (newValue: number) => {
        setSliderValue(newValue);
        onChange?.(newValue.toString());
    };

    return (
        <div className='flex flex-col gap-2'>
            <div className='flex flex-row justify-between items-center'>
                <Typography variant='body2'>{t('enhancements.intensity')}</Typography>
                <TextField
                    value={value}
                    onChange={onTextChange}
                    className='w-20 m-0'
                    slotProps={{
                        input: {
                            // biome-ignore lint/style/noJsxLiterals: symbol, not translatable copy
                            endAdornment: <Typography>%</Typography>,
                        },
                    }}
                />
            </div>

            <ValueSlider value={sliderValue} min={min} max={max} step={step} marks={marks} onChange={onSliderChange} />
        </div>
    );
};
