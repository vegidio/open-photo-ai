import { useEffect, useRef, useState } from 'react';
import { Events } from '@wailsio/runtime';
import { Initialize } from '@/bindings/gui/services/appservice.ts';
import { Drawer } from '@/components/Drawer';
import { DialogDownload } from '@/components/organisms/DialogDownload';
import { DialogTensorRT } from '@/components/organisms/DialogTensorRT';
import { Navbar } from '@/components/organisms/Navbar';
import { Preview } from '@/components/organisms/Preview';
import { Sidebar } from '@/components/Sidebar';
import { useSettingsStore } from '@/stores';

export const App = () => {
    const isFirstTensorRT = useSettingsStore((state) => state.isFirstTensorRT);
    const setProcessorSelectItems = useSettingsStore((state) => state.setProcessorSelectItems);

    const containerRef = useRef<HTMLDivElement>(null);
    const [isContainerReady, setIsContainerReady] = useState(false);
    const [openDownload, setOpenDownload] = useState(false);
    const [openTensorRT, setOpenTensorRT] = useState(false);

    useEffect(() => {
        if (containerRef.current) setIsContainerReady(true);
    }, []);

    // biome-ignore lint/correctness/useExhaustiveDependencies: N/A
    useEffect(() => {
        Events.Once('app:download', (_) => setOpenDownload(true));
        Events.Once('app:download:error', (_) => setOpenDownload(true));

        const initDependencies = async () => {
            try {
                const supportedEps = await Initialize();
                setProcessorSelectItems(supportedEps);
                setOpenDownload(false);

                // If it's the first run and TensorRT is supported, open the TensorRT dialog.
                if (supportedEps.TensorRT && isFirstTensorRT) setOpenTensorRT(true);
            } catch {
                console.error('Failed to initialize the app');
            }
        };

        initDependencies();

        return () => {
            Events.Off('app:download');
            Events.Off('app:download:error');
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

            <DialogDownload open={openDownload} onClose={() => setOpenDownload(false)} />

            <DialogTensorRT open={openTensorRT} onClose={() => setOpenTensorRT(false)} />
        </div>
    );
};
