import { useEffect, useState } from 'react';
import { AppBar, Toolbar, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { IsOutdated } from '@/bindings/gui/services/appservice';
import { Button } from '@/components/atoms/Button';
import { DialogAbout } from '@/features/navbar/DialogAbout';
import { NavbarCurrentFile } from '@/features/navbar/NavbarCurrentFile';
import { NavbarDimensions } from '@/features/navbar/NavbarDimensions';
import { Settings } from '@/features/settings';
import { useCurrentFile } from '@/hooks';
import { os, version } from '@/utils/constants.ts';

export const Navbar = () => {
    const currentFile = useCurrentFile();

    const [openSettings, setOpenSettings] = useState(false);
    const [openAbout, setOpenAbout] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState(false);

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

                        <Button option='text' size='small' onClick={() => setOpenSettings(true)}>
                            Settings
                        </Button>

                        <Button option='text' size='small' onClick={() => setOpenAbout(true)}>
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

            {openAbout && <DialogAbout open={true} onClose={() => setOpenAbout(false)} />}

            {openSettings && <Settings section='application' open={true} onClose={() => setOpenSettings(false)} />}
        </>
    );
};
