import { Button as MuiButton, type ButtonProps as MuiButtonProps } from '@mui/material';

type ButtonProps = MuiButtonProps & {
    option?: 'primary' | 'secondary' | 'tertiary';
};

export const Button = ({ option = 'primary', className = '', ...props }: ButtonProps) => {
    switch (option) {
        case 'primary':
            return (
                <MuiButton
                    variant='contained'
                    className={`bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] normal-case font-normal ${className}`}
                    {...props}
                />
            );

        case 'secondary':
            return (
                <MuiButton
                    variant='outlined'
                    className={`text-[#f2f2f2] normal-case font-normal ${className}`}
                    {...props}
                />
            );

        case 'tertiary':
            return (
                <MuiButton
                    variant='text'
                    color='inherit'
                    className={`text-[#f2f2f2] normal-case font-normal ${className}`}
                    {...props}
                />
            );
    }
};
