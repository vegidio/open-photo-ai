import { useMemo } from 'react';
import { ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { useAddEnhancements, useCurrentFile, useFileOperations } from '@/hooks';
import { useSettingsStore } from '@/stores';
import {
    ENHANCEMENTS,
    type EnhancementType,
    getCbOp,
    getDnOp,
    getFrOp,
    getLaOp,
    getShOp,
    getUpOp,
} from '@/utils/enhancement';

// The order the menu lists enhancements in. Separate from ENHANCEMENTS because object key order is not something to
// rely on for presentation.
const ENHANCEMENT_ORDER: EnhancementType[] = ['dn', 'fr', 'la', 'cb', 'sh', 'up'];

type MenuAddEnhancementProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onMenuClose: () => void;
};

export const MenuAddEnhancement = ({ anchorEl, open, onMenuClose }: MenuAddEnhancementProps) => {
    const { t } = useTranslation();
    const dnModel = useSettingsStore((state) => state.dnModel);
    const frModel = useSettingsStore((state) => state.frModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const cbModel = useSettingsStore((state) => state.cbModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const shModel = useSettingsStore((state) => state.shModel);

    const currentFile = useCurrentFile();
    const operations = useFileOperations(currentFile);
    const addEnhancements = useAddEnhancements();

    const onAddEnhancement = (op: Operation) => {
        if (currentFile) addEnhancements(currentFile, [op]);
        onMenuClose();
    };

    const defaultEnhancements = useMemo(() => {
        const [width, height] = currentFile?.Dimensions ?? [0, 0];
        const mp = width * height;
        const scale = mp <= 1_048_576 ? 4 : mp <= 4_194_304 ? 2 : 1;

        // Name and icon come from the ENHANCEMENTS registry; only the operation each row builds is specific to this
        // menu, since that is what depends on the user's default model for the enhancement.
        const ops: Record<EnhancementType, Operation> = {
            dn: getDnOp(dnModel),
            fr: getFrOp(frModel),
            la: getLaOp(laModel),
            cb: getCbOp(cbModel),
            sh: getShOp(shModel),
            up: getUpOp(upModel, scale),
        };

        return ENHANCEMENT_ORDER.map((type) => ({
            type,
            icon: <Icon option={ENHANCEMENTS[type].icon} />,
            name: t(ENHANCEMENTS[type].nameKey),
            op: ops[type],
        }));
    }, [dnModel, frModel, laModel, upModel, currentFile?.Dimensions, cbModel, shModel, t]);

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
