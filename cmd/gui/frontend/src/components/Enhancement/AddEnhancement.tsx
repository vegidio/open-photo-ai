import { type MouseEvent, useState } from 'react';
import { Button, ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import { FiPlus } from 'react-icons/fi';
import { MdOpenInFull, MdOutlineFaceRetouchingNatural } from 'react-icons/md';
import { Athens, Kyoto, type Operation } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';

type AddEnhancementProps = {
    disabled?: boolean;
};

const EMPTY_OPERATIONS: Operation[] = [];

export const AddEnhancement = ({ disabled = false }: AddEnhancementProps) => {
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
                disabled={disabled}
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

const options = [
    {
        type: 'fr',
        icon: <MdOutlineFaceRetouchingNatural />,
        name: 'Face Recovery',
        op: new Athens('fp32'),
    },
    {
        type: 'up',
        icon: <MdOpenInFull />,
        name: 'Upscale',
        op: new Kyoto('general', 4, 'fp32'),
    },
];

const EnhancementsMenu = ({ anchorEl, open, onMenuClose }: EnhancementsMenuProps) => {
    const currentFile = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) =>
        currentFile ? (state.enhancements.get(currentFile) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );
    const addEnhancements = useEnhancementStore((state) => state.addEnhancements);

    const onAddEnhancement = (op: Operation) => {
        addEnhancements(currentFile, [op]);
        onMenuClose();
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
            slotProps={{
                paper: {
                    style: {
                        marginLeft: '-2.5rem',
                    },
                },
            }}
        >
            {options.map((option) => {
                const exists = operations.some((op) => op.id.startsWith(option.type));

                return (
                    <MenuItem
                        key={option.name}
                        disabled={exists}
                        className='min-h-12'
                        onClick={() => onAddEnhancement(option.op)}
                    >
                        <ListItemIcon className='min-w-9 [&>svg]:size-5'>{option.icon}</ListItemIcon>
                        <ListItemText slotProps={{ primary: { className: 'text-[13px]' } }}>{option.name}</ListItemText>
                    </MenuItem>
                );
            })}
        </Menu>
    );
};
