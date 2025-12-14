import { AppBar, Button, Toolbar, Typography } from '@mui/material';

type NavbarProps = {
    className?: string;
};

export const Navbar = ({ className = '' }: NavbarProps) => {
    return (
        <AppBar position='static'>
            <Toolbar className='min-h-12 pl-24'>
                <Typography className='grow mt-1'>Open Photo AI</Typography>
                <Button color='inherit' disabled className='mt-0.5'>
                    Preferences
                </Button>
            </Toolbar>
        </AppBar>
    );
};
