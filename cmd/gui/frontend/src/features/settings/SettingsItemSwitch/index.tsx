import { ListItem, Switch, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';

type SettingsItemSwitchProps = TailwindProps & {
    id?: string;
    title: string;
    description?: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
};

export const SettingsItemSwitch = ({
    id,
    title,
    description,
    checked,
    onChange,
    className = '',
}: SettingsItemSwitchProps) => {
    return (
        <ListItem id={id} divider={true} className={`${className} py-3`}>
            <div className='flex flex-col flex-1 gap-2'>
                <div className='settings-row'>
                    <Typography variant='body2' className='flex-1 text-content-secondary'>
                        {title}
                    </Typography>

                    <Switch size='small' checked={checked} onChange={(e) => onChange(e.target.checked)} />
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
