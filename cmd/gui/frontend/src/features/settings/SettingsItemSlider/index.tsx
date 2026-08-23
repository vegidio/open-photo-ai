import { ListItem, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { ValueSlider } from '@/components/atoms/ValueSlider';

type SettingsItemSliderProps = TailwindProps & {
    id?: string;
    title: string;
    description?: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    marks?: { value: number; label: string }[];
    onChange: (value: number) => void;
};

export const SettingsItemSlider = ({
    id,
    title,
    description,
    value,
    min,
    max,
    step,
    marks,
    onChange,
    className = '',
}: SettingsItemSliderProps) => {
    return (
        <ListItem id={id} divider={true} className={`${className} pt-2 pb-3`}>
            <div className='flex flex-col flex-1 gap-2'>
                <div className='flex flex-row flex-1 items-center justify-between gap-4'>
                    <Typography variant='body2' className='flex-1 text-[#b0b0b0]'>
                        {title}
                    </Typography>

                    {/* Fixed width rather than flex-1: the mark labels hang below the track, and a slider that
                        stretches with the row leaves them colliding with the description on narrow layouts. */}
                    <ValueSlider
                        value={value}
                        min={min}
                        max={max}
                        step={step}
                        marks={marks}
                        onChange={onChange}
                        className='w-56 pb-0 mr-1.5'
                    />
                </div>

                {description && (
                    <Typography variant='caption' className='flex-1'>
                        {description}
                    </Typography>
                )}
            </div>
        </ListItem>
    );
};
