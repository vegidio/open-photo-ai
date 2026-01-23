import { useCallback, useEffect, useState } from 'react';
import { LinearProgress, Paper, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';

export const EnhancementProgress = () => {
    const [progress, setProgress] = useState({ name: 'Enhancing', value: 0 });

    const getOperationName = useCallback((id: string) => {
        switch (true) {
            case id.startsWith('dl'):
                return 'Downloading';
            case id.startsWith('fr'):
                return 'Face Recovery';
            case id.startsWith('la'):
                return 'Light Adjust';
            case id.startsWith('up'):
                return 'Upscale';
            default:
                return 'Enhancing';
        }
    }, []);

    useEffect(() => {
        Events.On('app:progress', (event) => {
            const [id, value] = event.data as [string, number];
            setProgress({ name: getOperationName(id), value: value * 100 });
        });

        return () => Events.Off('app:progress');
    }, [getOperationName]);

    return (
        <Paper
            elevation={8}
            className='absolute flex top-4 right-4 w-32 h-7 items-center justify-center rounded-lg z-10'
            sx={{
                backgroundImage: 'none',
            }}
        >
            <LinearProgress variant='determinate' value={progress.value} className='size-full rounded-[5px]' />
            <Typography variant='subtitle2' className='absolute text-gray-700'>
                {progress.name}
            </Typography>
        </Paper>
    );
};
