import { TextField as MuiTextField, type TextFieldProps as MuiTextFieldProps } from '@mui/material';

export const TextField = ({ ...props }: MuiTextFieldProps) => {
    return (
        <MuiTextField
            variant='outlined'
            size='small'
            margin='dense'
            slotProps={{
                input: {
                    className: 'text-sm bg-[#171717]',
                },
                inputLabel: {
                    className: 'text-sm',
                },
                htmlInput: {
                    autoCapitalize: 'off',
                    autoCorrect: 'off',
                },
            }}
            {...props}
        />
    );
};
