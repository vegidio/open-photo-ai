import { Divider, Link, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import icon from '@/assets/icon.avif';
import { DialogGeneral } from '@/components/molecules/DialogGeneral';
import { APP_COPYRIGHT, APP_NAME, APP_REPOSITORY, APP_WEBSITE, version } from '@/utils/constants';

type DialogAboutProps = {
    open: boolean;
    onClose: () => void;
};

export const DialogAbout = ({ open, onClose }: DialogAboutProps) => {
    const { t } = useTranslation();

    return (
        <DialogGeneral title={t('navbar.about')} open={open} onClose={onClose} className='w-96'>
            <div className='flex flex-col p-6 pt-2.5 gap-4 items-center'>
                <img src={icon} alt={t('navbar.about_dialog.iconAlt')} className='size-36' />

                <div className='flex flex-col gap-1 items-center'>
                    <Typography variant='h5' className='font-bold'>
                        {APP_NAME}
                    </Typography>
                    <Typography variant='body2' className='text-[#b0b0b0]'>
                        {t('navbar.about_dialog.version', { version })}
                    </Typography>
                </div>

                <div className='flex flex-col mt-2 gap-1 items-center text-[#b0b0b0]'>
                    <Typography className='text-sm'>{APP_COPYRIGHT}</Typography>

                    <div className='flex flex-row gap-2'>
                        <Link href='#' className='text-sm' onClick={() => Browser.OpenURL(APP_REPOSITORY.url)}>
                            {APP_REPOSITORY.label}
                        </Link>

                        <Divider orientation='vertical' flexItem className='bg-[#b0b0b0] my-0.5' />

                        <Link href='#' className='text-sm' onClick={() => Browser.OpenURL(APP_WEBSITE.url)}>
                            {APP_WEBSITE.label}
                        </Link>
                    </div>
                </div>
            </div>
        </DialogGeneral>
    );
};
