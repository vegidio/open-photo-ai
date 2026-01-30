import { ListItem, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { Select, type SelectItem } from '@/components/atoms/Select';

type SettingsItemSelectProps = TailwindProps & {
    title: string;
    description?: string;
    items: SelectItem[];
    selected: string;
    onSelect: (value: string) => void;
};

export const SettingsItemSelect = ({
    title,
    description,
    items,
    selected,
    onSelect,
    className = '',
}: SettingsItemSelectProps) => {
    return (
        <ListItem divider={true} className={`${className} py-3`}>
            <div className='flex flex-col flex-1 gap-2'>
                <div className='flex flex-row flex-1 items-center justify-between gap-4'>
                    <Typography variant='body2' className='flex-1 text-[#b0b0b0]'>
                        {title}
                    </Typography>

                    <Select items={items} value={selected} className='flex-1' onValueChange={onSelect} />
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
