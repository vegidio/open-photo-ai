import { type MouseEvent, useEffect, useState } from 'react';
import { Divider, IconButton, Menu, MenuItem, Typography } from '@mui/material';
import path from 'path-browserify';
import { IoIosMore } from 'react-icons/io';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import type { DialogFile } from '../../../bindings/gui/types';
import { useFileListStore, useFileStore } from '@/stores';
import { getImage } from '@/utils/image.ts';

type FileListItemProps = {
    file: DialogFile;
    selected?: boolean;
    onClick?: () => void;
};

export const FileListItem = ({ file, selected = false, onClick }: FileListItemProps) => {
    const [imageUrl, setImageUrl] = useState<string>();

    useEffect(() => {
        async function loadImage() {
            const imageUrl = await getImage(file, 100);
            setImageUrl(imageUrl);
        }

        loadImage();
    }, [file]);

    return (
        // Using a div instead of a button here to avoid nested buttons error
        // biome-ignore lint/a11y/noStaticElementInteractions: N/A
        // biome-ignore lint/a11y/useKeyWithClickEvents: N/A
        <div
            onClick={onClick}
            className={`h-full aspect-square rounded ${selected ? 'outline-3 outline-blue-500' : ''}`}
        >
            <div className='relative w-full h-full'>
                <img alt='Preview' src={imageUrl} className='w-full h-full object-cover rounded' />

                <BottomBar file={file} selected={selected} className='absolute bottom-0 left-0 right-0 h-5 rounded-b' />
            </div>
        </div>
    );
};

type BottomBarProps = TailwindProps & {
    file: DialogFile;
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
    file: DialogFile;
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

const OptionsMenu = ({ file, anchorEl, open, onMenuClose }: OptionsMenuProps) => {
    const removeFile = useFileStore((state) => state.removeFile);
    const clear = useFileStore((state) => state.clear);
    const toggle = useFileListStore((state) => state.toggle);

    const onCloseImage = () => {
        removeFile(file.Hash);
        onMenuClose();
    };

    const onCloseAllImages = () => {
        clear();
        onMenuClose();
        toggle();
    };

    return (
        <Menu anchorEl={anchorEl} open={open} onClose={onMenuClose}>
            <MenuItem onClick={onCloseImage}>Close image</MenuItem>
            <MenuItem onClick={onCloseAllImages}>Close all images</MenuItem>
            <Divider />
            <MenuItem onClick={onMenuClose}>Show in Finder</MenuItem>
        </Menu>
    );
};
