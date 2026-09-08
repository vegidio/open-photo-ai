import { Divider, ListItemText, Menu, MenuItem, type PopoverOrigin } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { useFileManager, useNotify } from '@/hooks';
import { useDrawerStore, useFileStore } from '@/stores';
import { os } from '@/utils/constants.ts';

type MenuFileOptionsProps = {
    file: File;
    anchorEl: HTMLElement | undefined;
    anchorOrigin: PopoverOrigin;
    transformOrigin: PopoverOrigin;
    open: boolean;
    onMenuClose: () => void;
};

export const MenuFileOptions = ({
    file,
    anchorEl,
    anchorOrigin,
    transformOrigin,
    open,
    onMenuClose,
}: MenuFileOptionsProps) => {
    const { t } = useTranslation();
    const { enqueueSnackbar } = useNotify();
    const { removeFile, clearAll } = useFileManager();
    const setOpen = useDrawerStore((state) => state.setOpen);

    const updateDrawer = () => {
        onMenuClose();
        if (useFileStore.getState().files.length === 0) setOpen(false);
    };

    const onCloseImage = () => {
        removeFile(file);
        updateDrawer();
    };

    const onCloseAllImages = () => {
        clearAll();
        updateDrawer();
    };

    // Guarded rather than left floating, matching the export queue's reveal button and the log-file button in
    // Settings: revealing can fail on a file that was moved or deleted since it was opened, and silently doing nothing
    // reads as a broken menu item. The menu closes either way - the click was handled.
    const onReveal = async () => {
        onMenuClose();

        try {
            await RevealInFileManager(file.Path);
        } catch (e) {
            console.error('Failed to reveal the file', e);
            enqueueSnackbar(t('errors.revealFileFailed'), { variant: 'error' });
        }
    };

    const options = [
        { name: t('menu.file.close'), action: onCloseImage },
        { name: t('menu.file.closeAll'), action: onCloseAllImages },
        { name: undefined },
        // i18next context rather than interpolating the file manager's name: the whole sentence has to be one
        // translatable unit, since word order around an app name isn't universal. An undefined context falls back
        // to the base key, so there's no _linux variant to keep in sync.
        {
            name: t('menu.file.showIn', {
                context: os === 'darwin' ? 'darwin' : os === 'windows' ? 'windows' : undefined,
            }),
            action: onReveal,
        },
    ];

    return (
        <Menu
            anchorEl={anchorEl}
            open={open}
            onClose={onMenuClose}
            anchorOrigin={anchorOrigin}
            transformOrigin={transformOrigin}
        >
            {options.map((option) =>
                option.name ? (
                    <MenuItem key={option.name} onClick={option.action}>
                        <ListItemText slotProps={{ primary: { className: 'text-[13px]' } }}>{option.name}</ListItemText>
                    </MenuItem>
                ) : (
                    <Divider key='divider' />
                ),
            )}
        </Menu>
    );
};
