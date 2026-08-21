import { useCallback, useEffect, useState } from 'react';
import { LinearProgress, Paper, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';

export const EnhancementProgress = () => {
    const { t } = useTranslation();
    const [progress, setProgress] = useState({ name: t('preview.progress.enhancing'), value: 0 });

    const getOperationName = useCallback(
        (id: string, phase: string, fraction: number) => {
            // The download is the one phase whose own percentage is worth spelling out: the bar tracks the whole
            // pipeline, so a download barely moves it, and without the number it reads as if nothing is happening.
            if (phase === 'download') {
                return t('preview.progress.downloading', { percent: Math.round(fraction * 100) });
            }

            switch (true) {
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
            const { name, phase, progress, fraction } = event.data;
            setProgress({ name: getOperationName(name, phase, fraction), value: progress * 100 });
        });
    }, [getOperationName]);

    return (
        <Paper
            elevation={8}
            className='bg-none absolute flex top-4 right-4 w-36 h-7 items-center justify-center rounded-lg z-10'
        >
            <LinearProgress variant='determinate' value={progress.value} className='size-full rounded-[5px]' />
            {/* Stretched to the Paper's box (inset-0) instead of relying on the flex static position: an inset-less
                absolute child gets a shrink-to-fit width bounded by the space left of its centered static position,
                which wraps two-word labels that would otherwise fit the full width. */}
            <Typography
                variant='subtitle2'
                className='absolute inset-0 flex items-center justify-center whitespace-nowrap px-1 text-gray-700'
            >
                {progress.name}
            </Typography>
        </Paper>
    );
};
