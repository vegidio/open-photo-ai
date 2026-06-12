import { useState } from 'react';
import { MenuItem, Select, type SelectChangeEvent, Typography } from '@mui/material';
import { DialogService } from '@/bindings/gui/services';
import { useExportStore } from '@/stores';

type LocationType = 'hidden' | 'original' | 'browse';

export const ExportSettingsLocation = () => {
    const location = useExportStore((state) => state.location);
    const setLocation = useExportStore((state) => state.setLocation);
    const [value, setValue] = useState<LocationType>(location ? 'hidden' : 'original');

    const handleChange = async (event: SelectChangeEvent) => {
        const newValue = event.target.value;

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
                value={value}
                onChange={handleChange}
                size='small'
                renderValue={() => (value === 'hidden' ? location : 'Original directory')}
                slotProps={{
                    input: {
                        className: 'text-sm',
                    },
                }}
            >
                {/* This hidden item is chosen when a directory is selected */}
                <MenuItem value='hidden' className='hidden' />

                <MenuItem value='original' className='text-sm'>
                    Original directory
                </MenuItem>
                <MenuItem value='browse' className='text-sm'>
                    Browse...
                </MenuItem>
            </Select>
        </div>
    );
};
