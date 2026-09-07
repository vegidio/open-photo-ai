import { Button as MuiButton, type ButtonProps as MuiButtonProps } from '@mui/material';

type ButtonProps = MuiButtonProps & {
    option?: 'primary' | 'secondary' | 'tertiary' | 'text';
};

export const Button = ({ option = 'primary', className = '', ...props }: ButtonProps) => {
    switch (option) {
        case 'primary':
            return (
                <MuiButton
                    variant='contained'
                    className={`bg-brand hover:bg-brand-hover text-content-primary normal-case font-normal whitespace-nowrap ${className}`}
                    {...props}
                />
            );

        case 'secondary':
            return (
                <MuiButton
                    variant='contained'
                    className={`bg-surface-overlay hover:bg-surface-base text-content-primary normal-case font-normal whitespace-nowrap ${className}`}
                    {...props}
                />
            );

        case 'tertiary':
            return (
                <MuiButton
                    variant='outlined'
                    className={`text-content-primary normal-case font-normal whitespace-nowrap ${className}`}
                    {...props}
                />
            );

        case 'text':
            return (
                <MuiButton
                    variant='text'
                    color='inherit'
                    className={`text-content-primary normal-case font-normal whitespace-nowrap ${className}`}
                    {...props}
                />
            );
    }
};
