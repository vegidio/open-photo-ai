import { useEffect, useMemo, useState } from 'react';
import { Button, Dialog, Divider, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { Initialize } from '@/bindings/gui/services/appservice.ts';
import { DownloadAnimation } from '@/components/molecules/DownloadAnimation';
import { DownloadProgress } from '@/components/molecules/DownloadProgress';

type DownloadDialogProps = {
    open: boolean;
    onClose: () => void;
};

export const DownloadDialog = ({ open, onClose }: DownloadDialogProps) => {
    const [downloads, setDownloads] = useState<Record<string, number>>({});
    const [error, setError] = useState(false);

    useEffect(() => {
        Events.On('app:download', (event) => {
            const [dependency, progress] = event.data as [string, number];
            setDownloads({ ...downloads, [dependency]: progress });
        });

        return () => Events.Off('app:download');
    }, [downloads]);

    useEffect(() => {
        Events.On('app:download:error', (_) => setError(true));
        return () => Events.Off('app:download:error');
    }, []);

    const { message1, message2 } = useMemo(() => {
        if (error) {
            return {
                message1: 'Error downloading the dependencies.',
                message2: 'The app cannot start until all dependencies are downloaded!',
            };
        } else {
            return {
                message1: 'Downloading dependencies...',
                message2: 'Please wait!',
            };
        }
    }, [error]);

    const onTryAgain = async () => {
        setError(false);

        try {
            await Initialize();
            onClose();
        } catch {
            console.error('Failed to initialize the app');
        }
    };

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
                <DownloadAnimation />

                <div className='flex flex-col items-center gap-0.5'>
                    <Typography>{message1}</Typography>
                    <Typography>{message2}</Typography>
                </div>

                <Divider className='w-full' />

                {Object.entries(downloads).map(([name, progress]) => {
                    return <DownloadProgress key={name} name={name} value={progress * 100} className='w-full' />;
                })}

                <Button color='error' disabled={!error} onClick={onTryAgain}>
                    {error ? 'Try again' : 'Working...'}
                </Button>
            </div>
        </Dialog>
    );
};
