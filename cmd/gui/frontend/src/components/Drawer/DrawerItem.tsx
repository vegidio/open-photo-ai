import { type ChangeEvent, type MouseEvent, useEffect, useState } from 'react';
import { Checkbox, Divider, IconButton, ListItemText, Menu, MenuItem, Typography } from '@mui/material';
import path from 'path-browserify';
import { IoIosMore } from 'react-icons/io';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import type { File } from '../../../bindings/gui/types';
import { RevealInFileManager } from '../../../bindings/gui/services/osservice.ts';
import { useDrawerStore, useEnhancementStore, useFileStore } from '@/stores';
import { os } from '@/utils/constants.ts';
import { getImage } from '@/utils/image.ts';

type FileListItemProps = {
    file: File;
    current?: boolean;
    onClick?: () => void;
};

export const DrawerItem = ({ file, current = false, onClick }: FileListItemProps) => {
    const isSelected = useFileStore((state) =>
        state.selectedFiles.some((selectedFile) => selectedFile.Path === file.Path),
    );
    const addSelectedFile = useFileStore((state) => state.addSelectedFile);
    const removeSelectedFile = useFileStore((state) => state.removeSelectedFile);

    const [image, setImage] = useState<string>();

    const handleCheckboxChange = (event: ChangeEvent<HTMLInputElement>) => {
        const checked = event.target.checked;

        if (checked) {
            addSelectedFile(file);
        } else {
            removeSelectedFile(file.Path);
        }
    };

    useEffect(() => {
        async function loadImage() {
            const imageData = await getImage(file, 100);
            setImage(imageData.url);
        }

        loadImage();
    }, [file]);

    return (
        // Using a div instead of a button here to avoid nested buttons error
        // biome-ignore lint/a11y/noStaticElementInteractions: N/A
        // biome-ignore lint/a11y/useKeyWithClickEvents: N/A
        <div
            onClick={onClick}
            className={`h-full aspect-square rounded ${current ? 'outline-3 outline-blue-500' : ''}`}
        >
            <div className='relative size-full'>
                <img alt='Preview' src={image} className='size-full object-cover rounded' />

                <Checkbox
                    size='small'
                    checked={isSelected}
                    onChange={handleCheckboxChange}
                    className='absolute top-0 right-0 p-0 m-0.5 rounded bg-black/50 text-white'
                />

                <BottomBar file={file} selected={current} className='absolute bottom-0 left-0 right-0 h-5 rounded-b' />
            </div>
        </div>
    );
};

type BottomBarProps = TailwindProps & {
    file: File;
    selected?: boolean;
};

const BottomBar = ({ file, selected = false, className = '' }: BottomBarProps) => {
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
    const open = Boolean(anchorEl);
    const fileName = path.basename(file.Path);

    const onMenuOpen = (event: MouseEvent<HTMLButtonElement>) => {
        // Prevent click from bubbling to the parent button
        event.stopPropagation();
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(null);
    };

    return (
        <>
            <div className={`flex items-center p-2 gap-1 bg-white/75 ${!selected ? 'invert' : ''} ${className}`}>
                <Typography variant='caption' className='text-left truncate flex-1 text-black'>
                    {fileName}
                </Typography>

                <IconButton type='button' disableRipple className='p-0' onClick={onMenuOpen}>
                    <IoIosMore className='size-4 text-black' />
                </IconButton>
            </div>

            <OptionsMenu file={file} anchorEl={anchorEl} open={open} onMenuClose={onMenuClose} />
        </>
    );
};

type OptionsMenuProps = {
    file: File;
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

const OptionsMenu = ({ file, anchorEl, open, onMenuClose }: OptionsMenuProps) => {
    const removeFile = useFileStore((state) => state.removeFile);
    const clear = useFileStore((state) => state.clear);
    const setOpen = useDrawerStore((state) => state.setOpen);

    const enhancementRemoveFile = useEnhancementStore((state) => state.removeFile);
    const enhancementClearFiles = useEnhancementStore((state) => state.clearFiles);

    const updateDrawer = () => {
        onMenuClose();
        if (useFileStore.getState().files.length === 0) setOpen(false);
    };

    const onCloseImage = () => {
        removeFile(file.Path);
        enhancementRemoveFile(file);
        updateDrawer();
    };

    const onCloseAllImages = () => {
        clear();
        enhancementClearFiles();
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
            anchorOrigin={{
                vertical: 'center',
                horizontal: 'center',
            }}
            transformOrigin={{
                vertical: 'bottom',
                horizontal: 'left',
            }}
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
