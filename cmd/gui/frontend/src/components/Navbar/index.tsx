import { useEffect, useState } from 'react';
import { AppBar, Toolbar, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { IsOutdated } from '../../../bindings/gui/services/appservice.ts';
import { NavbarAbout } from './NavbarAbout.tsx';
import { Button } from '@/components/atoms/Button';
import { os, version } from '@/utils/constants.ts';

export const Navbar = () => {
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
                    <Typography className='grow mt-1'>Open Photo AI</Typography>

                    <div className='mt-0.5 flex flex-row items-center gap-3'>
                        <Button option='text' size='small' onClick={onAboutClick}>
                            About
                        </Button>

                        <Typography variant='caption' className='text-[#545454] mt-0.5'>
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
