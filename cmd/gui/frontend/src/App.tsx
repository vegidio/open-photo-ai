import { useEffect, useRef, useState } from 'react';
import { Events } from '@wailsio/runtime';
import { Initialize } from '../bindings/gui/services/appservice.ts';
import { Download } from '@/components/Download';
import { Drawer } from '@/components/Drawer';
import { Navbar } from '@/components/Navbar';
import { Preview } from '@/components/Preview';
import { Sidebar } from '@/components/Sidebar';

export const App = () => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [isContainerReady, setIsContainerReady] = useState(false);
    const [openDownload, setOpenDownload] = useState(false);

    useEffect(() => {
        if (containerRef.current) {
            setIsContainerReady(true);
        }
    }, []);

    useEffect(() => {
        Events.On('app:download', (_) => {
            setOpenDownload(true);
        });

        async function init() {
            try {
                await Initialize();
                setOpenDownload(false);
            } catch {
                console.error('Failed to initialize the app');
            }
        }

        init();

        return () => Events.Off('app:download');
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

            <Download open={openDownload} onClose={() => setOpenDownload(false)} />
        </div>
    );
};
