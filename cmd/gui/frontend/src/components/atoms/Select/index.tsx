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

    // Draws a rule under this item, marking the end of a group.
    //
    // It sits on the item above the break rather than being an entry of its own because MUI's Select clones every
    // element child into a role="option" with a click handler and a data-value: a <Divider/> or <ListSubheader> child
    // becomes a phantom option, announced to screen readers and counted in the out-of-range-value warning, saved from
    // being selectable only by the incidental fact that it carries no tabindex. `divider` is a style on a real option,
    // so none of that applies.
    divider?: boolean;
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
            className={`bg-surface-base ${className ?? ''}`}
            slotProps={{
                input: {
                    className: 'text-sm',
                },
            }}
            // `bg-none` cancels the white overlay that MUI paints on elevated Paper; without it the dropdown stays grey.
            MenuProps={{
                slotProps: {
                    paper: {
                        className: 'bg-surface-base bg-none border border-surface-header',
                    },
                },
            }}
            {...props}
        >
            {items.map(({ value, label, disabled = false, hidden = false, divider = false }) => (
                <MenuItem
                    key={value}
                    value={value}
                    disabled={disabled}
                    divider={divider}
                    className={hidden ? 'hidden' : 'text-sm'}
                >
                    {label}
                </MenuItem>
            ))}
        </MuiSelect>
    );
};
