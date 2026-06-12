import { type MouseEvent, type ReactNode, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { OptionsColorBalance } from '@/features/enhancements/OptionsColorBalance';
import { OptionsDenoise } from '@/features/enhancements/OptionsDenoise';
import { OptionsFaceRecovery } from '@/features/enhancements/OptionsFaceRecovery';
import { OptionsLightAdjustment } from '@/features/enhancements/OptionsLightAdjustment';
import { OptionsSharpen } from '@/features/enhancements/OptionsSharpen';
import { OptionsUpscale } from '@/features/enhancements/OptionsUpscale';
import { useCurrentFile, useFileDisabledFaces, useFileFaces } from '@/hooks';
import { useEnhancementStore } from '@/stores';

type ListItemEnhancementProps = {
    op: Operation;
};

export const ListItemEnhancement = ({ op }: ListItemEnhancementProps) => {
    const [isHovered, setIsHovered] = useState(false);
    const file = useCurrentFile();
    const faces = useFileFaces(file);
    const disabledFaces = useFileDisabledFaces(file);
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);

    // Get enhancement details and options component menu
    const { name, info, icon } = opToEnhancement(op, facesLabel(faces.length, disabledFaces.size));
    const OptionsComponent = selectOptionsComponent(op.id);

    const [anchorEl, setAnchorEl] = useState<HTMLElement | undefined>(undefined);
    const open = Boolean(anchorEl);

    const onMenuOpen = (event: MouseEvent<HTMLDivElement>) => {
        setAnchorEl(event.currentTarget);
    };

    const onMenuClose = () => {
        setAnchorEl(undefined);
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
                    ) : undefined
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
        case operationId.startsWith('dn'):
            return OptionsDenoise;

        case operationId.startsWith('fr'):
            return OptionsFaceRecovery;

        case operationId.startsWith('la'):
            return OptionsLightAdjustment;

        case operationId.startsWith('cb'):
            return OptionsColorBalance;

        case operationId.startsWith('sh'):
            return OptionsSharpen;

        case operationId.startsWith('up'):
            return OptionsUpscale;
    }
};

const opToEnhancement = (op: Operation, faceText: string): { name: string; info: string; icon: ReactNode } => {
    const quality = op.options.precision === 'fp32' ? 'High' : 'Std.';

    switch (true) {
        // Denoise
        case op.id.startsWith('dn'): {
            const info = `${titleCase(op.options.name)}, ${quality}`;
            return { name: 'Denoise', info, icon: <Icon option='denoise' /> };
        }

        // Face Recovery
        case op.id.startsWith('fr'): {
            const info = `${titleCase(op.options.name)}, ${faceText}, ${quality}`;
            return { name: 'Face Recovery', info, icon: <Icon option='face_recovery' /> };
        }

        // Light Adjustment
        case op.id.startsWith('la'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = `${titleCase(op.options.name)}, ${intensity}%, ${quality}`;
            return { name: 'Light Adjustment', info, icon: <Icon option='light_adjustment' /> };
        }

        // Color Balance
        case op.id.startsWith('cb'): {
            const intensity = parseFloat(op.options.intensity) * 100;
            const info = `${titleCase(op.options.name)}, ${intensity}%, ${quality}`;
            return { name: 'Color Balance', info, icon: <Icon option='color_balance' /> };
        }

        // Upscale
        case op.id.startsWith('up'): {
            const scale = parseFloat(parseFloat(op.options.scale).toFixed(3));
            const info = `${titleCase(op.options.name)}, ${scale}x, ${quality}`;
            return { name: 'Upscale', info, icon: <Icon option='upscale' /> };
        }

        // Sharpen
        case op.id.startsWith('sh'): {
            const info = `${titleCase(op.options.name)}, ${quality}`;
            return { name: 'Sharpen', info, icon: <Icon option='sharpen' /> };
        }
    }

    return { name: '', info: '', icon: undefined };
};

const titleCase = (input: string): string => {
    if (!input) return input;
    return input[0].toUpperCase() + input.slice(1);
};

const facesLabel = (total: number, disabledCount: number): string => {
    const enabled = Math.max(0, total - disabledCount);
    const noun = total === 1 ? 'Face' : 'Faces';
    return enabled === total ? `${total} ${noun}` : `${enabled}/${total} ${noun}`;
};
