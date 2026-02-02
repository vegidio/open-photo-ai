import { Divider, Link, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import icon from '@/assets/icon.avif';
import { DialogGeneral } from '@/components/molecules/DialogGeneral';
import { version } from '@/utils/constants';

type DialogAboutProps = {
    open: boolean;
    onClose: () => void;
};

export const DialogAbout = ({ open, onClose }: DialogAboutProps) => {
    return (
        <DialogGeneral title='About' open={open} onClose={onClose} className='w-96'>
            <div className='flex flex-col p-6 pt-2.5 gap-4 items-center'>
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
                    <Typography className='text-sm'>© 2025—2026, Vinicius Egidio</Typography>

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
        </DialogGeneral>
    );
};
