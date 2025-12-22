import { useState } from 'react';
import { AppBar, Button, Toolbar, Typography } from '@mui/material';
import { Version } from '../../../bindings/gui/services/appservice.ts';
import { NavbarAbout } from '@/components/Navbar/NavbarAbout.tsx';

type NavbarProps = {
    className?: string;
};

const version = await Version();

export const Navbar = ({ className = '' }: NavbarProps) => {
    const [openAbout, setOpenAbout] = useState(false);

    const onAboutClick = () => {
        setOpenAbout(true);
    };

    return (
        <>
            <AppBar position='static'>
                <Toolbar className='min-h-12 pl-24'>
                    <Typography className='grow mt-1'>Open Photo AI</Typography>

                    <div className='mt-0.5 flex flex-row items-center gap-3'>
                        <Button color='inherit' onClick={onAboutClick} className='normal-case font-normal'>
                            About
                        </Button>

                        <Typography variant='caption' className='text-[#545454] mt-0.5'>
                            v{version}
                        </Typography>
                    </div>
                </Toolbar>
            </AppBar>

            <NavbarAbout version={version} open={openAbout} onClose={() => setOpenAbout(false)} />
        </>
    );
};
