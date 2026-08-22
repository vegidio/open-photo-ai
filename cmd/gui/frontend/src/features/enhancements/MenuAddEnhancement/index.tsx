import { useMemo } from 'react';
import { ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { useAddEnhancements, useCurrentFile, useFileOperations } from '@/hooks';
import { useSettingsStore } from '@/stores';
import { defaultUpscaleScale, ENHANCEMENT_ORDER, ENHANCEMENTS, getOp } from '@/utils/enhancement';

type MenuAddEnhancementProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onMenuClose: () => void;
};

export const MenuAddEnhancement = ({ anchorEl, open, onMenuClose }: MenuAddEnhancementProps) => {
    const { t } = useTranslation();
    const models = useSettingsStore((state) => state.models);

    const currentFile = useCurrentFile();
    const operations = useFileOperations(currentFile);
    const addEnhancements = useAddEnhancements();

    const onAddEnhancement = (op: Operation) => {
        if (currentFile) addEnhancements(currentFile, [op]);
        onMenuClose();
    };

    const defaultEnhancements = useMemo(() => {
        // Name, icon and the operation itself all come from the ENHANCEMENTS registry; the only thing specific to this
        // menu is that the operation is built at the user's default model, and upscale at a scale fitting the image.
        const scale = defaultUpscaleScale(currentFile);

        return ENHANCEMENT_ORDER.map((type) => ({
            type,
            icon: <Icon option={ENHANCEMENTS[type].icon} />,
            name: t(ENHANCEMENTS[type].nameKey),
            op: getOp(type, models[type], type === 'up' ? scale : undefined),
        }));
    }, [models, currentFile, t]);

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
