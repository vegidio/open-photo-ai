import { useEffect, useState } from 'react';
import { AppBar, Button, Toolbar, Typography } from '@mui/material';
import { IsOutdated } from '../../../bindings/gui/services/appservice.ts';
import { NavbarAbout } from './NavbarAbout.tsx';
import { NavbarUpdate } from './NavbarUpdate.tsx';
import { os, version } from '@/utils/constants.ts';

export const Navbar = () => {
    const [openAbout, setOpenAbout] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState(false);

    const onAboutClick = () => {
        setOpenAbout(true);
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
                        <Button color='inherit' size='small' onClick={onAboutClick} className='normal-case font-normal'>
                            About
                        </Button>

                        <Typography variant='caption' className='text-[#545454] mt-0.5'>
                            v{version}
                        </Typography>

                        {updateAvailable && <NavbarUpdate className='ml-1' />}
                    </div>
                </Toolbar>
            </AppBar>

            <NavbarAbout open={openAbout} onClose={() => setOpenAbout(false)} />
        </>
    );
};
