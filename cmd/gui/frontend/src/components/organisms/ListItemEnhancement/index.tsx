import { type MouseEvent, type ReactNode, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { OptionsFaceRecovery } from '@/components/organisms/OptionsFaceRecovery';
import { OptionsLightAdjustment } from '@/components/organisms/OptionsLightAdjustment';
import { OptionsUpscale } from '@/components/organisms/OptionsUpscale';
import { useEnhancementStore, useFileStore } from '@/stores';

type ListItemEnhancementProps = {
    op: Operation;
};

export const ListItemEnhancement = ({ op }: ListItemEnhancementProps) => {
    const [isHovered, setIsHovered] = useState(false);
    const file = useFileStore((state) => state.files.at(state.currentIndex));
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);

    // Get enhancement details and options component menu
    const { name, info, icon } = opToEnhancement(op);
    const OptionsComponent = selectOptionsComponent(op.id);

    const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLDivElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(null);
    };

    const onRemove = () => {
        if (file) removeEnhancement(file, op.id);
    };

    return (
        <>
            <ListItem
                divider={true}
                disablePadding
                onMouseEnter={() => setIsHovered(true)}
                onMouseLeave={() => setIsHovered(false)}
                secondaryAction={
                    isHovered ? (
                        <IconButton disableRipple edge='end' onClick={onRemove}>
                            <Icon option='close' />
                        </IconButton>
                    ) : null
                }
            >
                <ListItemButton className='min-h-12' onClick={onMenuOpen}>
                    <ListItemIcon className='min-w-9 [&>svg]:size-5'>{icon}</ListItemIcon>
                    <ListItemText
                        primary={name}
                        secondary={info}
                        className='my-0'
                        slotProps={{
                            primary: {
                                className: 'text-[13px] text-white',
                            },
                            secondary: {
                                className: 'text-[13px] text-[#545454] italic',
                            },
                        }}
                    />
                </ListItemButton>
            </ListItem>

            {open && OptionsComponent && <OptionsComponent anchorEl={anchorEl} open={true} onClose={onMenuClose} />}
        </>
    );
};

const selectOptionsComponent = (operationId: string) => {
    switch (true) {
        case operationId.startsWith('fr'):
            return OptionsFaceRecovery;

        case operationId.startsWith('la'):
            return OptionsLightAdjustment;

        case operationId.startsWith('up'):
            return OptionsUpscale;
    }
};

const opToEnhancement = (op: Operation): { name: string; info: string; icon: ReactNode } => {
    const quality = `${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;

    switch (true) {
        // Face Recovery
        case op.id.startsWith('fr'): {
            const info = `${titleCase(op.options.name)}, ${quality}`;
            return { name: 'Face Recovery', info, icon: <Icon option='face_recovery' /> };
        }

        // Light Adjustment
        case op.id.startsWith('la'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = `${titleCase(op.options.name)}, ${intensity}%, ${quality}`;
            return { name: 'Light Adjustment', info, icon: <Icon option='light_adjustment' /> };
        }

        // Upscale
        case op.id.startsWith('up'): {
            const scale = parseFloat(parseFloat(op.options.scale).toFixed(3));
            const info = `${titleCase(op.options.name)}, ${scale}x, ${quality}`;
            return { name: 'Upscale', info, icon: <Icon option='upscale' /> };
        }
    }

    return { name: '', info: '', icon: null };
};

const titleCase = (input: string): string => {
    if (!input) return input;
    return input[0].toUpperCase() + input.slice(1);
};
