import { useMemo, useState } from 'react';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { DialogService } from '@/bindings/gui/services';
import { Select, type SelectItem } from '@/components/atoms/Select';
import { useExportStore } from '@/stores';

type LocationType = 'hidden' | 'original' | 'browse';

export const ExportSettingsLocation = () => {
    const { t } = useTranslation();
    const location = useExportStore((state) => state.location);
    const setLocation = useExportStore((state) => state.setLocation);
    const [value, setValue] = useState<LocationType>(location ? 'hidden' : 'original');

    // Built here rather than at module scope, so the labels follow a language change instead of being frozen at
    // module-evaluation time.
    const items = useMemo<SelectItem[]>(
        () => [
            // This hidden item is chosen when a directory is selected
            { value: 'hidden', label: '', hidden: true },
            { value: 'original', label: t('export.settings.location.original') },
            { value: 'browse', label: t('export.settings.location.browse') },
        ],
        [t],
    );

    const handleChange = async (newValue: string) => {
        if (newValue !== 'browse') {
            setValue(newValue as LocationType);
            setLocation(undefined);
            return;
        }

        try {
            // The native dialog's title is translated here: the backend has no i18n catalog, so the caller supplies
            // the copy.
            const path = await DialogService.OpenDirDialog(t('dialogs.native.selectDirectory'));

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
                {t('export.settings.location.title')}
            </Typography>

            <Select
                items={items}
                value={value}
                onValueChange={handleChange}
                renderValue={() => (value === 'hidden' ? location : t('export.settings.location.original'))}
            />
        </div>
    );
};
