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
import i18n from '@/i18n';
import { useSettingsStore } from '@/stores';
import { getErrorMessage } from '@/utils/errors.ts';

export const App = () => {
    const { enqueueSnackbar } = useNotify();

    const isFirstTensorRT = useSettingsStore((state) => state.isFirstTensorRT);
    const setProcessorOptions = useSettingsStore((state) => state.setProcessorOptions);

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

            // Two complete sentences behind a ternary rather than one built from a shared tail: the clause about
            // the processor and the clause about the consequence have to be free to reorder in translation.
            //
            // i18n.t rather than useTranslation's t throughout this effect: it's registered once with an empty
            // dependency list, and adding t would tear down and re-register every Wails listener on a language change.
            const message =
                provider && provider !== ExecutionProvider.ExecutionProviderAuto
                    ? i18n.t('toasts.cpuFallbackProvider', { provider })
                    : i18n.t('toasts.cpuFallbackAuto');

            enqueueSnackbar(message, {
                variant: 'warning',
                autoHideDuration: 10000,
            });
        });

        // Emitted when a drag-and-drop included files the decoder can't read. The backend sends only the base names
        // and leaves the wording here, where the catalog can pick the right plural form for the active language.
        const offUnsupported = Events.On('app:unsupportedFiles', (event) => {
            const names: string[] = event.data?.names ?? [];
            if (names.length === 0) return;

            enqueueSnackbar(i18n.t('toasts.unsupportedFiles', { count: names.length, names: names.join(', ') }), {
                variant: 'warning',
                autoHideDuration: 10000,
            });
        });

        const initDependencies = async () => {
            try {
                const supportedEps = await Initialize();
                setProcessorOptions(supportedEps);
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
            offUnsupported();
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
