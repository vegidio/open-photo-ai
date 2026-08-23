import { useEffect, useMemo, useState } from 'react';
import { Button, Dialog, Divider, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import { Initialize } from '@/bindings/gui/services/appservice.ts';
import { DownloadAnimation } from '@/features/dialogs/DownloadAnimation';
import { DownloadProgress } from '@/features/dialogs/DownloadProgress';

type DialogDownloadProps = {
    open: boolean;
    hasError?: boolean;
    onClose: () => void;
};

export const DialogDownload = ({ open, hasError = false, onClose }: DialogDownloadProps) => {
    const { t } = useTranslation();
    const [downloads, setDownloads] = useState<Record<string, number>>({});
    const [error, setError] = useState(false);

    useEffect(() => {
        // Returns the per-listener unsubscribe; `Events.Off('app:download')` would also tear down App's listener.
        return Events.On('app:download', (event) => {
            const { dependency, percent } = event.data;
            setDownloads((prev) => ({ ...prev, [dependency]: percent }));
        });
    }, []);

    useEffect(() => {
        if (hasError) setError(true);
    }, [hasError]);

    const { message1, message2 } = useMemo(() => {
        if (error) {
            return {
                message1: t('dialogs.download.error'),
                message2: t('dialogs.download.errorDetail'),
            };
        } else {
            return {
                message1: t('dialogs.download.downloading'),
                message2: t('dialogs.download.pleaseWait'),
            };
        }
    }, [error, t]);

    const onTryAgain = async () => {
        setError(false);

        try {
            await Initialize();
            onClose();
        } catch (e) {
            // The retry leaves the dialog in its error state, so the only record of *why* the retry failed - which is
            // usually a different reason from the first attempt - is this line.
            console.error('Retrying initialization failed', e);
            setError(true);
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
                    {error ? t('dialogs.download.tryAgain') : t('dialogs.download.working')}
                </Button>
            </div>
        </Dialog>
    );
};
