import { Divider, ListItemText, Menu, MenuItem, type PopoverOrigin } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { useDrawerStore, useFileStore } from '@/stores';
import { os } from '@/utils/constants.ts';

type MenuFileOptionsProps = {
    file: File;
    anchorEl: HTMLElement | null;
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
    const removeFile = useFileStore((state) => state.removeFile);
    const clear = useFileStore((state) => state.clear);
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
        clear();
        updateDrawer();
    };

    const onReveal = () => {
        RevealInFileManager(file.Path);
        onMenuClose();
    };

    const fmName = os === 'darwin' ? 'Finder' : os === 'windows' ? 'Explorer' : 'File Manager';

    const options = [
        { name: 'Close image', action: onCloseImage },
        { name: 'Close all images', action: onCloseAllImages },
        { name: undefined },
        { name: `Show in ${fmName}`, action: onReveal },
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
