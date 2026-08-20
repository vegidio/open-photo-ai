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
    disabled?: boolean;
    hidden?: boolean;
};

type SelectProps = MuiSelectProps<string> &
    TailwindProps & {
        items: SelectItem[];
        onValueChange?: (value: string) => void;
    };

export const Select = ({ items, onValueChange, className, ...props }: SelectProps) => {
    const handleChange = (event: SelectChangeEvent) => {
        onValueChange?.(event.target.value);
    };

    return (
        <MuiSelect
            onChange={handleChange}
            size='small'
            className={`bg-[#171717] ${className ?? ''}`}
            slotProps={{
                input: {
                    className: 'text-sm',
                },
            }}
            // `bg-none` cancels the white overlay that MUI paints on elevated Paper; without it the dropdown stays grey.
            MenuProps={{
                slotProps: {
                    paper: {
                        className: 'bg-[#171717] bg-none border border-[#2b2b2b]',
                    },
                },
            }}
            {...props}
        >
            {items.map(({ value, label, disabled = false, hidden = false }) => (
                <MenuItem key={value} value={value} disabled={disabled} className={hidden ? 'hidden' : 'text-sm'}>
                    {label}
                </MenuItem>
            ))}
        </MuiSelect>
    );
};
