import type { ChangeEvent } from 'react';
import { Checkbox, FormControlLabel } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useFileStore } from '@/stores';

type DrawerSelectAllProps = TailwindProps & {
    disabled?: boolean;
};

export const DrawerSelectAll = ({ disabled = false, className = '' }: DrawerSelectAllProps) => {
    const state = useFileStore((state) => {
        if (state.selectedFiles.length > 0) {
            return state.files.length === state.selectedFiles.length ? 'all' : 'indeterminate';
        }

        return 'none';
    });

    const selectAll = useFileStore((state) => state.selectAll);
    const unselectAll = useFileStore((state) => state.unselectAll);

    const onSelectAll = (event: ChangeEvent<HTMLInputElement>) => {
        const checked = event.target.checked;

        if (checked) {
            selectAll();
        } else {
            unselectAll();
        }
    };

    return (
        <FormControlLabel
            control={
                <Checkbox
                    disableRipple
                    size='small'
                    checked={state === 'all'}
                    indeterminate={state === 'indeterminate'}
                    onChange={onSelectAll}
                />
            }
            label={state === 'all' ? 'Unselect all' : 'Select all'}
            disabled={disabled}
            className={`${className}`}
            slotProps={{
                typography: {
                    className: 'text-sm',
                },
            }}
        />
    );
};
