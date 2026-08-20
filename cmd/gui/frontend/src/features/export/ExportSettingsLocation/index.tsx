import { useState } from 'react';
import { Typography } from '@mui/material';
import { DialogService } from '@/bindings/gui/services';
import { Select, type SelectItem } from '@/components/atoms/Select';
import { useExportStore } from '@/stores';

type LocationType = 'hidden' | 'original' | 'browse';

const items: SelectItem[] = [
    // This hidden item is chosen when a directory is selected
    { value: 'hidden', label: '', hidden: true },
    { value: 'original', label: 'Original directory' },
    { value: 'browse', label: 'Browse...' },
];

export const ExportSettingsLocation = () => {
    const location = useExportStore((state) => state.location);
    const setLocation = useExportStore((state) => state.setLocation);
    const [value, setValue] = useState<LocationType>(location ? 'hidden' : 'original');

    const handleChange = async (newValue: string) => {
        if (newValue !== 'browse') {
            setValue(newValue as LocationType);
            setLocation(undefined);
            return;
        }

        try {
            const path = await DialogService.OpenDirDialog();

            if (path) {
                setValue('hidden');
                setLocation(path);
            } else if (!location) {
                setValue('original');
            }
        } catch (e) {
            console.error('Error choosing directory', e);
            setValue('original');
        }
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                Save to
            </Typography>

            <Select
                items={items}
                value={value}
                onValueChange={handleChange}
                renderValue={() => (value === 'hidden' ? location : 'Original directory')}
            />
        </div>
    );
};
