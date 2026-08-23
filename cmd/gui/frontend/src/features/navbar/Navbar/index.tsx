import { useEffect, useState } from 'react';
import { AppBar, Toolbar, Typography } from '@mui/material';
import { Browser } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import { AnalyticsEvent, track } from '@/analytics';
import { IsOutdated } from '@/bindings/gui/services/appservice';
import { Button } from '@/components/atoms/Button';
import { DialogAbout } from '@/features/navbar/DialogAbout';
import { NavbarCurrentFile } from '@/features/navbar/NavbarCurrentFile';
import { NavbarDimensions } from '@/features/navbar/NavbarDimensions';
import { Settings } from '@/features/settings';
import { useCurrentFile } from '@/hooks';
import { APP_NAME, os, version } from '@/utils/constants.ts';

export const Navbar = () => {
    const { t } = useTranslation();
    const currentFile = useCurrentFile();

    const [openSettings, setOpenSettings] = useState(false);
    const [openAbout, setOpenAbout] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState(false);

    const onUpdateClick = () => {
        Browser.OpenURL('https://github.com/vegidio/open-photo-ai/releases');
    };

    useEffect(() => {
        // Guarded rather than left floating: IsOutdated makes a network call, and until now a rejection here became an
        // unhandled rejection with the update indicator simply never appearing.
        IsOutdated()
            .then((outdated) => {
                setUpdateAvailable(outdated);

                // How many users are running a build we have already superseded, which is the denominator for every
                // "is this fixed in the latest version?" support answer.
                if (outdated) track(AnalyticsEvent.UpdateAvailable, { version });
            })
            .catch((e) => console.error('Update check failed', e));
    }, []);

    return (
        <>
            <AppBar position='static'>
                <Toolbar className={`min-h-12 ${os === 'darwin' ? 'pl-[86px]' : ''}`}>
                    {/* Left side */}
                    <div className='flex flex-row items-center mt-1 h-full grow'>
                        <Typography>{APP_NAME}</Typography>

                        {currentFile && <NavbarCurrentFile file={currentFile} className='ml-4' />}
                    </div>

                    {/* Right side */}
                    <div className='mt-1 flex flex-row h-full items-center gap-3'>
                        {currentFile && <NavbarDimensions file={currentFile} />}

                        <Button
                            option='text'
                            size='small'
                            onClick={() => {
                                track(AnalyticsEvent.SettingsOpened);
                                setOpenSettings(true);
                            }}
                        >
                            {t('settings.title')}
                        </Button>

                        <Button option='text' size='small' onClick={() => setOpenAbout(true)}>
                            {t('navbar.about')}
                        </Button>

                        <Typography variant='caption' className='text-[#545454]'>
                            {t('navbar.version', { version })}
                        </Typography>

                        {updateAvailable && (
                            <Button size='small' onClick={onUpdateClick} className='ml-1 animate-pulse'>
                                {t('navbar.updateAvailable')}
                            </Button>
                        )}
                    </div>
                </Toolbar>
            </AppBar>

            {openAbout && <DialogAbout open={true} onClose={() => setOpenAbout(false)} />}

            {openSettings && <Settings open={true} onClose={() => setOpenSettings(false)} />}
        </>
    );
};
