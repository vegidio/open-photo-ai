import { useEffect, useState } from 'react';
import { Box, Button, CircularProgress, Dialog, Divider, LinearProgress, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { MdCloudDownload } from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type DownloadProps = {
    open: boolean;
    onClose: () => void;
};

export const Download = ({ open, onClose }: DownloadProps) => {
    const [downloads, setDownloads] = useState<Record<string, number>>({});

    useEffect(() => {
        Events.On('app:download', (event) => {
            const [dependency, progress] = event.data as [string, number];
            setDownloads({ ...downloads, [dependency]: progress });
        });

        return () => Events.Off('app:download');
    }, [downloads]);

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') {
                    onClose();
                }
            }}
            slotProps={{
                paper: {
                    className: 'bg-none bg-[#212121] w-[32rem] p-6 overflow-hidden',
                },
            }}
        >
            <div className='flex flex-col items-center gap-4.5'>
                <CircularProgressIcon />

                <div className='flex flex-col items-center gap-0.5'>
                    <Typography>Downloading dependencies...</Typography>
                    <Typography>Please wait!</Typography>
                </div>

                <Divider className='w-full' />

                {Object.entries(downloads).map(([name, progress]) => {
                    return <DependencyProgress key={name} name={name} value={progress * 100} className='w-full' />;
                })}

                <Button disabled={true}>Working...</Button>
            </div>
        </Dialog>
    );
};

const CircularProgressIcon = ({ className = '' }: TailwindProps) => {
    return (
        <Box className={`${className} relative flex items-center justify-center w-fit`}>
            <CircularProgress variant='indeterminate' size={60} />
            <MdCloudDownload className='absolute size-7' />
        </Box>
    );
};

type DependencyProgressProps = TailwindProps & {
    name: string;
    value: number;
};

const DependencyProgress = ({ name, value, className = '' }: DependencyProgressProps) => {
    return (
        <div className={`${className} flex flex-row gap-2 items-center`}>
            <Typography variant='body2' className='w-28'>
                {name}
            </Typography>

            <div className='flex flex-row flex-1 items-center'>
                <LinearProgress variant='determinate' value={value} className='flex-1' />
                <Typography variant='caption' align='right' className='text-[#b0b0b0] w-10'>
                    {value.toFixed(0)}%
                </Typography>
            </div>
        </div>
    );
};
