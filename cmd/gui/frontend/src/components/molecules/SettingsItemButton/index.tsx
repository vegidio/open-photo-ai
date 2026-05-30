import { ListItem, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { Button } from '@/components/atoms/Button';

type SettingsItemButtonProps = TailwindProps & {
    id?: string;
    title: string;
    description?: string;
    button: string;
    onClick: () => void;
};

export const SettingsItemButton = ({
    id,
    title,
    description,
    button,
    onClick,
    className = '',
}: SettingsItemButtonProps) => {
    return (
        <ListItem id={id} divider={true} className={`${className} py-3`}>
            <div className='flex flex-col flex-1 gap-2'>
                <div className='flex flex-row flex-1 items-center justify-between gap-4'>
                    <Typography variant='body2' className='flex-1 text-[#b0b0b0]'>
                        {title}
                    </Typography>

                    <Button option='tertiary' className='flex-1 ml-7' onClick={onClick}>
                        {button}
                    </Button>
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
