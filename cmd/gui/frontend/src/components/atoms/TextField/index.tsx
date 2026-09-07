import { TextField as MuiTextField, type TextFieldProps as MuiTextFieldProps } from '@mui/material';

export const TextField = ({ slotProps, ...props }: MuiTextFieldProps) => {
    return (
        <MuiTextField
            variant='outlined'
            size='small'
            margin='dense'
            slotProps={{
                input: {
                    className: 'text-sm bg-surface-base',
                    ...slotProps?.input,
                },
                inputLabel: {
                    className: 'text-sm',
                    ...slotProps?.inputLabel,
                },
                htmlInput: {
                    autoCapitalize: 'off',
                    autoCorrect: 'off',
                    ...slotProps?.htmlInput,
                },
            }}
            {...props}
        />
    );
};
