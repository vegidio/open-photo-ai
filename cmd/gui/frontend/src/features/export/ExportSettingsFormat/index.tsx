import { useMemo } from 'react';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Select, type SelectItem } from '@/components/atoms/Select';
import { useExportStore } from '@/stores';

export const ExportSettingsFormat = () => {
    const { t } = useTranslation();
    const format = useExportStore((state) => state.format);
    const setFormat = useExportStore((state) => state.setFormat);

    // Built here rather than at module scope: 'Preserve' is the only real word in the list, and a module-level array
    // would capture whatever language was active when this file was first evaluated. The rest are format acronyms.
    const items = useMemo<SelectItem[]>(
        () => [
            { value: 'preserve', label: t('export.settings.format.preserve') },
            { value: 'avif', label: 'AVIF' },
            { value: 'bmp', label: 'BMP' },
            { value: 'gif', label: 'GIF' },
            { value: 'heic', label: 'HEIC' },
            { value: 'jpg', label: 'JPG' },
            { value: 'png', label: 'PNG' },
            { value: 'tiff', label: 'TIFF' },
            { value: 'webp', label: 'WEBP' },
        ],
        [t],
    );

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                {t('export.settings.format.title')}
            </Typography>

            <Select items={items} value={format} onValueChange={setFormat} />
        </div>
    );
};
