import { type MouseEvent, useState } from 'react';
import { Button, Menu, MenuItem } from '@mui/material';
import { FiPlus } from 'react-icons/fi';

export const AddEnhancement = () => {
    const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLButtonElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(null);
    };

    return (
        <>
            <Button
                variant='outlined'
                className='normal-case text-white font-normal'
                startIcon={<FiPlus className='size-6 stroke-1' />}
                onClick={onMenuOpen}
            >
                Add enhancement
            </Button>

            <EnhancementsMenu anchorEl={anchorEl} open={open} onMenuClose={onMenuClose} />
        </>
    );
};

type EnhancementsMenuProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

const EnhancementsMenu = ({ anchorEl, open, onMenuClose }: EnhancementsMenuProps) => {
    const updateDrawer = () => {
        onMenuClose();
    };

    const onCloseImage = () => {
        updateDrawer();
    };

    const onCloseAllImages = () => {
        updateDrawer();
    };

    return (
        <Menu
            anchorEl={anchorEl}
            open={open}
            onClose={onMenuClose}
            anchorOrigin={{
                vertical: 'center',
                horizontal: 'left',
            }}
            transformOrigin={{
                vertical: 'top',
                horizontal: 'right',
            }}
        >
            <MenuItem onClick={onCloseImage}>Face Recovery</MenuItem>
            <MenuItem onClick={onCloseAllImages}>Upscale</MenuItem>
        </Menu>
    );
};
