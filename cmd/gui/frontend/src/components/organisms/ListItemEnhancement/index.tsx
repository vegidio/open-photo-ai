import { type MouseEvent, type ReactNode, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { OptionsFaceRecovery } from '@/components/organisms/OptionsFaceRecovery';
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
    const { name, config, icon } = opToEnhancement(op);
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
                        secondary={config}
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

        case operationId.startsWith('up'):
            return OptionsUpscale;
    }
};

const opToEnhancement = (op: Operation): { name: string; config: string; icon: ReactNode } => {
    let config = '';

    switch (true) {
        // Face Recovery
        case op.id.startsWith('fr'): {
            config = `${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;
            return { name: 'Face Recovery', config, icon: <Icon option='face_recovery' /> };
        }

        // Upscale
        case op.id.startsWith('up_tokyo'): {
            config = `${op.options.scale}x`;
            config += `, ${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;
            return { name: 'Upscale', config, icon: <Icon option='upscale' /> };
        }

        case op.id.startsWith('up_kyoto'): {
            config = op.options.mode === 'general' ? 'General' : 'Cartoon';
            config += `, ${op.options.scale}x`;
            config += `, ${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;
            return { name: 'Upscale', config, icon: <Icon option='upscale' /> };
        }
    }

    return { name: '', config: '', icon: null };
};
