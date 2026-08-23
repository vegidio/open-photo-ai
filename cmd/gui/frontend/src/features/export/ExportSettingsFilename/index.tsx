import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { TextField } from '@/components/atoms/TextField';
import { Toggle } from '@/components/atoms/Toggle';
import { useExportStore } from '@/stores';

export const ExportSettingsFilename = () => {
    const { t } = useTranslation();
    const prefix = useExportStore((state) => state.prefix);
    const setPrefix = useExportStore((state) => state.setPrefix);
    const suffix = useExportStore((state) => state.suffix);
    const setSuffix = useExportStore((state) => state.setSuffix);
    const overwrite = useExportStore((state) => state.overwrite);
    const setOverwrite = useExportStore((state) => state.setOverwrite);

    return (
        <div className='flex flex-col'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                {t('export.settings.filename.title')}
            </Typography>

            <TextField
                label={t('export.settings.filename.prefix')}
                value={prefix}
                onChange={(e) => setPrefix(e.target.value)}
            />

            <TextField
                label={t('export.settings.filename.suffix')}
                value={suffix}
                onChange={(e) => setSuffix(e.target.value)}
            />

            <Toggle
                label={
                    <Typography variant='body2' className='text-[#b0b0b0]'>
                        {t('export.settings.filename.allowOverwrite')}
                    </Typography>
                }
                value={overwrite}
                color='#009aff'
                onChange={(value) => setOverwrite(value)}
                className='mt-1'
            />

            <Typography variant='caption' className={`${overwrite ? 'text-[#ffcc00]' : ''} mt-1.5`}>
                {t(
                    overwrite
                        ? 'export.settings.filename.sameLocationOverwrite'
                        : 'export.settings.filename.sameLocationRename',
                )}
            </Typography>
        </div>
    );
};
