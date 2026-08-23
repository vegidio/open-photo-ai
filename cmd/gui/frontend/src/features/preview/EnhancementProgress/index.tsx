import { useEffect, useMemo, useState } from 'react';
import { LinearProgress, Paper, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import { Phase } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { ENHANCEMENTS, getEnhancementType } from '@/utils/enhancement';

type ProgressState = {
    operation: string;
    phase: Phase | '';
    fraction: number;
    value: number;
};

const INITIAL: ProgressState = { operation: '', phase: '', fraction: 0, value: 0 };

export const EnhancementProgress = () => {
    const { t } = useTranslation();
    const [progress, setProgress] = useState<ProgressState>(INITIAL);

    useEffect(() => {
        // Returns the per-listener unsubscribe; `Events.Off` is global across every listener for the name.
        //
        // Only the raw event is stored, never a translated label: keeping t() out of here is what lets the dependency
        // list stay empty, so a language change doesn't tear down and re-register the listener.
        return Events.On('app:progress', (event) => {
            const { name, phase, progress, fraction } = event.data;
            const value = Math.round(progress * 100);

            // This fires once per tile - thousands of times on a large upscale - but the bar only has 100 distinct
            // positions. Returning the previous object when nothing visible changed bails React out of the re-render.
            setProgress((prev) =>
                prev.value === value && prev.operation === name && prev.phase === phase
                    ? prev
                    : { operation: name, phase, fraction, value },
            );
        });
    }, []);

    const name = useMemo(() => {
        // The download is the one phase whose own percentage is worth spelling out: the bar tracks the whole
        // pipeline, so a download barely moves it, and without the number it reads as if nothing is happening.
        if (progress.phase === Phase.PhaseDownload) {
            return t('preview.progress.downloading', { percent: Math.round(progress.fraction * 100) });
        }

        const type = getEnhancementType(progress.operation);
        const enhancement = type && ENHANCEMENTS[type];

        return enhancement ? t(enhancement.shortNameKey) : t('preview.progress.enhancing');
    }, [progress.phase, progress.fraction, progress.operation, t]);

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
                {name}
            </Typography>
        </Paper>
    );
};
