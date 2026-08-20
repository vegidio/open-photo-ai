import { useEffect, useRef, useState } from 'react';
import { Events } from '@wailsio/runtime';
import { AnalyticsEvent, track } from '@/analytics';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { Initialize } from '@/bindings/gui/services/appservice.ts';
import { DialogDownload, DialogTensorRT } from '@/features/dialogs';
import { Drawer } from '@/features/drawer';
import { Navbar } from '@/features/navbar';
import { Preview } from '@/features/preview';
import { Sidebar } from '@/features/sidebar';
import { useNotify } from '@/hooks';
import { useSettingsStore } from '@/stores';
import { getErrorMessage } from '@/utils/errors.ts';

export const App = () => {
    const { enqueueSnackbar } = useNotify();

    const isFirstTensorRT = useSettingsStore((state) => state.isFirstTensorRT);
    const setProcessorSelectItems = useSettingsStore((state) => state.setProcessorSelectItems);

    const containerRef = useRef<HTMLDivElement>(null);
    const [isContainerReady, setIsContainerReady] = useState(false);
    const [openDownload, setOpenDownload] = useState(false);
    const [downloadError, setDownloadError] = useState(false);
    const [openTensorRT, setOpenTensorRT] = useState(false);

    useEffect(() => {
        if (containerRef.current) setIsContainerReady(true);
    }, []);

    // biome-ignore lint/correctness/useExhaustiveDependencies: N/A
    useEffect(() => {
        // `Events.Off(name)` removes *every* listener for that name, including ones other components registered
        // (DialogDownload also listens for `app:download`). `On`/`Once` return an unsubscribe for just this listener,
        // which is what cleanup must use.
        const offDownload = Events.Once('app:download', (_) => setOpenDownload(true));

        // The backend downgrades to the CPU when the selected processor can't run a model (broken/outdated GPU
        // driver, no free VRAM). It's emitted once per run, so the message isn't repeated on every enhancement.
        const offFallback = Events.On('app:fallback', (event) => {
            const provider = event.data?.provider ?? '';
            track(AnalyticsEvent.ProviderFallback, { provider });

            const reason =
                provider && provider !== ExecutionProvider.ExecutionProviderAuto
                    ? `${provider} couldn't be used on this system`
                    : 'No GPU processor could be used on this system';

            enqueueSnackbar(`${reason}, so the CPU is being used instead. Enhancements will be slower.`, {
                variant: 'warning',
                autoHideDuration: 10000,
            });
        });

        const initDependencies = async () => {
            try {
                const supportedEps = await Initialize();
                setProcessorSelectItems(supportedEps);
                setOpenDownload(false);
                track(AnalyticsEvent.AppInitialized, {
                    execution_provider: useSettingsStore.getState().executionProvider,
                });

                if (supportedEps.TensorRT && isFirstTensorRT) setOpenTensorRT(true);
            } catch (e) {
                console.error('Failed to initialize the app');
                track(AnalyticsEvent.InitFailed, { reason: getErrorMessage(e) });
                setOpenDownload(true);
                setDownloadError(true);
            }
        };

        initDependencies();

        return () => {
            offDownload();
            offFallback();
        };
    }, []);

    return (
        <div className='flex h-screen flex-col'>
            <Navbar />

            <main className='flex flex-1 min-h-0 flex-row'>
                <div id='preview_filelist' ref={containerRef} className='flex-1 relative overflow-hidden'>
                    <Preview className='h-[calc(100%-48px)]' />

                    {isContainerReady && <Drawer containerRef={containerRef} />}
                </div>

                <Sidebar className='w-64 h-full' />
            </main>

            <DialogDownload
                open={openDownload}
                hasError={downloadError}
                onClose={() => {
                    setOpenDownload(false);
                    setDownloadError(false);
                }}
            />

            <DialogTensorRT open={openTensorRT} onClose={() => setOpenTensorRT(false)} />
        </div>
    );
};
