import { IconButton as MuiIconButton, type IconButtonProps as MuiIconButtonProps } from '@mui/material';
import { Icon, type IconName } from '@/components/atoms/Icon';

type IconButtonProps = MuiIconButtonProps & {
    option: IconName;
};

export const IconButton = ({ option, ...props }: IconButtonProps) => {
    return (
        <MuiIconButton {...props}>
            <Icon option={option} />
        </MuiIconButton>
    );
};
