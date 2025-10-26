import { useEffect, useRef, useState } from 'react';
import { Navbar } from './components/Navbar';
import { Preview } from './components/Preview';
import { Sidebar } from './components/Sidebar';
import { FileList } from '@/components/FileList';

export const App = () => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [isContainerReady, setIsContainerReady] = useState(false);

    useEffect(() => {
        if (containerRef.current) {
            setIsContainerReady(true);
        }
    }, []);

    return (
        <div className='flex h-screen flex-col'>
            <Navbar />

            <main className='flex flex-1 min-h-0 flex-row'>
                <div id='preview_filelist' ref={containerRef} className='flex-1 relative overflow-hidden'>
                    <Preview className='h-[calc(100%-48px)]' />

                    {isContainerReady && <FileList containerRef={containerRef} />}
                </div>

                <Sidebar className='w-64 h-full' />
            </main>
        </div>
    );
};
