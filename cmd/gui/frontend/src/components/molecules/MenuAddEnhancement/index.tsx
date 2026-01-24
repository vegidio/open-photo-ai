import { ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import { Icon } from '@/components/atoms/Icon';
import { Athens, Kyoto, type Operation, Paris } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

const options = [
    {
        type: 'fr',
        icon: <Icon option='face_recovery' />,
        name: 'Face Recovery',
        op: new Athens('fp32'),
    },
    {
        type: 'la',
        icon: <Icon option='light_adjustment' />,
        name: 'Light Adjustment',
        op: new Paris(0.5, 'fp32'),
    },
    {
        type: 'up',
        icon: <Icon option='upscale' />,
        name: 'Upscale',
        op: new Kyoto(4, 'fp32'),
    },
];

type MenuAddEnhancementProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

export const MenuAddEnhancement = ({ anchorEl, open, onMenuClose }: MenuAddEnhancementProps) => {
    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const operations = useEnhancementStore((state) =>
        currentFile ? (state.enhancements.get(currentFile) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );
    const addEnhancements = useEnhancementStore((state) => state.addEnhancements);

    const onAddEnhancement = (op: Operation) => {
        if (currentFile) addEnhancements(currentFile, [op]);
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
