import { useEffect, useState } from 'react';
import { AppBar, Toolbar, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { IsOutdated } from '@/bindings/gui/services/appservice.ts';
import { Button } from '@/components/atoms/Button';
import { NavbarAbout } from '@/components/molecules/NavbarAbout';
import { NavbarCurrentFile } from '@/components/molecules/NavbarCurrentFile';
import { NavbarDimensions } from '@/components/molecules/NavbarDimensions';
import { useFileStore } from '@/stores';
import { os, version } from '@/utils/constants.ts';

export const Navbar = () => {
    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));

    const [openAbout, setOpenAbout] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState(false);

    const onAboutClick = () => {
        setOpenAbout(true);
    };

    const onUpdateClick = () => {
        Browser.OpenURL('https://github.com/vegidio/open-photo-ai/releases');
    };

    useEffect(() => {
        IsOutdated().then(setUpdateAvailable);
    }, []);

    return (
        <>
            <AppBar position='static'>
                <Toolbar className={`min-h-12 ${os === 'darwin' ? 'pl-[86px]' : ''}`}>
                    {/* Left side */}
                    <div className='flex flex-row items-center mt-1 h-full grow'>
                        <Typography>Open Photo AI</Typography>

                        {currentFile && <NavbarCurrentFile file={currentFile} className='ml-4' />}
                    </div>

                    {/* Right side */}
                    <div className='mt-1 flex flex-row h-full items-center gap-3'>
                        {currentFile && <NavbarDimensions file={currentFile} />}

                        <Button option='text' size='small' onClick={onAboutClick}>
                            About
                        </Button>

                        <Typography variant='caption' className='text-[#545454]'>
                            v{version}
                        </Typography>

                        {updateAvailable && (
                            <Button size='small' onClick={onUpdateClick} className='ml-1 animate-pulse'>
                                Update Available
                            </Button>
                        )}
                    </div>
                </Toolbar>
            </AppBar>

            <NavbarAbout open={openAbout} onClose={() => setOpenAbout(false)} />
        </>
    );
};
