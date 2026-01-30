import {
    MenuItem,
    Select as MuiSelect,
    type SelectProps as MuiSelectProps,
    type SelectChangeEvent,
} from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';

export type SelectItem = {
    value: string;
    label: string;
};

type SelectProps = MuiSelectProps<string> &
    TailwindProps & {
        items: SelectItem[];
        onValueChange?: (value: string) => void;
    };

export const Select = ({ items, onValueChange, ...props }: SelectProps) => {
    const handleChange = (event: SelectChangeEvent) => {
        onValueChange?.(event.target.value);
    };

    return (
        <MuiSelect
            onChange={handleChange}
            size='small'
            slotProps={{
                input: {
                    className: 'text-sm',
                },
            }}
            {...props}
        >
            {items.map(({ value, label }) => (
                <MenuItem key={value} value={value} className='text-sm'>
                    {label}
                </MenuItem>
            ))}
        </MuiSelect>
    );
};
