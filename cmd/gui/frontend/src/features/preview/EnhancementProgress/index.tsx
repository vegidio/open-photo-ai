import { useCallback, useEffect, useState } from 'react';
import { LinearProgress, Paper, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';

export const EnhancementProgress = () => {
    const { t } = useTranslation();
    const [progress, setProgress] = useState({ name: t('preview.progress.enhancing'), value: 0 });

    const getOperationName = useCallback(
        (id: string) => {
            switch (true) {
                case id.startsWith('dl'):
                    return t('preview.progress.downloading');
                case id.startsWith('dn'):
                    return t('preview.progress.denoise');
                case id.startsWith('fr'):
                    return t('preview.progress.faceRecovery');
                case id.startsWith('la'):
                    return t('preview.progress.lightAdjustment');
                case id.startsWith('cb'):
                    return t('preview.progress.colorBalance');
                case id.startsWith('up'):
                    return t('preview.progress.upscale');
                case id.startsWith('sh'):
                    return t('preview.progress.sharpen');
                default:
                    return t('preview.progress.enhancing');
            }
        },
        [t],
    );

    useEffect(() => {
        // Returns the per-listener unsubscribe; `Events.Off` is global across every listener for the name.
        return Events.On('app:progress', (event) => {
            const { name, progress } = event.data;
            setProgress({ name: getOperationName(name), value: progress * 100 });
        });
    }, [getOperationName]);

    return (
        <Paper
            elevation={8}
            className='bg-none absolute flex top-4 right-4 w-32 h-7 items-center justify-center rounded-lg z-10'
        >
            <LinearProgress variant='determinate' value={progress.value} className='size-full rounded-[5px]' />
            <Typography variant='subtitle2' className='absolute text-gray-700'>
                {progress.name}
            </Typography>
        </Paper>
    );
};
