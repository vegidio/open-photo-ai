import { useEffect, useState } from 'react';
import { Button, Dialog, Divider, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { DownloadLoading } from '@/components/Download/DownloadLoading.tsx';
import { DownloadProgress } from '@/components/Download/DownloadProgress.tsx';

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
                <DownloadLoading />

                <div className='flex flex-col items-center gap-0.5'>
                    <Typography>Downloading dependencies...</Typography>
                    <Typography>Please wait!</Typography>
                </div>

                <Divider className='w-full' />

                {Object.entries(downloads).map(([name, progress]) => {
                    return <DownloadProgress key={name} name={name} value={progress * 100} className='w-full' />;
                })}

                <Button disabled={true}>Working...</Button>
            </div>
        </Dialog>
    );
};
