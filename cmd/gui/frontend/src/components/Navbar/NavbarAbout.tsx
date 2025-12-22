import { Dialog, DialogTitle, Divider, IconButton, Link, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { MdClose } from 'react-icons/md';
import icon from '@/assets/icon.avif';

type NavbarAboutProps = {
    version: string;
    open: boolean;
    onClose: () => void;
};

export const NavbarAbout = ({ version, open, onClose }: NavbarAboutProps) => {
    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') {
                    onClose();
                }
            }}
            slotProps={{
                paper: {
                    className: 'bg-[#212121] w-96 max-w-full',
                    sx: {
                        backgroundImage: 'none',
                    },
                },
            }}
        >
            <DialogTitle className='p-3 text-xs text-[#9e9e9e]'>About</DialogTitle>

            <IconButton
                onClick={onClose}
                sx={(theme) => ({
                    position: 'absolute',
                    right: 4,
                    top: 2,
                    color: theme.palette.grey[500],
                })}
            >
                <MdClose className='size-5' />
            </IconButton>

            <Divider />

            <div className='flex flex-col p-6 pt-4 gap-4 items-center'>
                <img src={icon} alt='App Icon' className='size-36' />

                <div className='flex flex-col gap-1 items-center'>
                    <Typography variant='h5' className='font-bold'>
                        Open Photo AI
                    </Typography>
                    <Typography variant='body2' className='text-[#b0b0b0]'>
                        Version {version}
                    </Typography>
                </div>

                <div className='flex flex-col mt-2 gap-1 items-center text-[#b0b0b0]'>
                    <Typography className='text-sm'>© 2025, Vinicius Egidio</Typography>

                    <div className='flex flex-row gap-2'>
                        <Link
                            href='#'
                            className='text-sm'
                            onClick={() => Browser.OpenURL('https://github.com/vegidio/open-photo-ai')}
                        >
                            Github
                        </Link>

                        <Divider orientation='vertical' flexItem className='bg-[#b0b0b0] my-0.5' />

                        <Link href='#' className='text-sm' onClick={() => Browser.OpenURL('https://vinicius.io')}>
                            vinicius.io
                        </Link>
                    </div>
                </div>
            </div>
        </Dialog>
    );
};
