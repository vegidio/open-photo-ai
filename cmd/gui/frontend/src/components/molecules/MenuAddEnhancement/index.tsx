import { useMemo } from 'react';
import { ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { useEnhancementStore, useFileStore, useSettingsStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';
import { getFrOp, getLaOp, getUpOp } from '@/utils/enhancement';

type MenuAddEnhancementProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

export const MenuAddEnhancement = ({ anchorEl, open, onMenuClose }: MenuAddEnhancementProps) => {
    const frModel = useSettingsStore((state) => state.frModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const upModel = useSettingsStore((state) => state.upModel);

    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const operations = useEnhancementStore((state) =>
        currentFile ? (state.enhancements.get(currentFile) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );
    const addEnhancements = useEnhancementStore((state) => state.addEnhancements);

    const onAddEnhancement = (op: Operation) => {
        if (currentFile) addEnhancements(currentFile, [op]);
        onMenuClose();
    };

    const defaultEnhancements = useMemo(() => {
        const [width, height] = currentFile?.Dimensions ?? [0, 0];
        const mp = width * height;
        const scale = mp <= 1_048_576 ? 4 : mp <= 4_194_304 ? 2 : 1;

        return [
            {
                type: 'fr',
                icon: <Icon option='face_recovery' />,
                name: 'Face Recovery',
                op: getFrOp(frModel),
            },
            {
                type: 'la',
                icon: <Icon option='light_adjustment' />,
                name: 'Light Adjustment',
                op: getLaOp(laModel),
            },
            {
                type: 'up',
                icon: <Icon option='upscale' />,
                name: 'Upscale',
                op: getUpOp(upModel, scale),
            },
        ];
    }, [frModel, laModel, upModel, currentFile?.Dimensions]);

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
            {defaultEnhancements.map((option) => {
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
