import { type ReactNode, useState } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import type { Operation } from '@/operations';
import { Icon } from '@/components/atoms/Icon';
import { useEnhancementStore, useFileStore } from '@/stores';

type EnhancementItemProps = {
    op: Operation;
};

export const EnhancementListItem = ({ op }: EnhancementItemProps) => {
    const [isHovered, setIsHovered] = useState(false);
    const file = useFileStore((state) => state.files.at(state.currentIndex));
    const removeEnhancement = useEnhancementStore((state) => state.removeEnhancement);
    const { name, config, icon } = opToEnhancement(op);

    const onRemove = () => {
        if (file) removeEnhancement(file, op.id);
    };

    return (
        <ListItem
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
            <ListItemButton className='min-h-12'>
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
    );
};

const opToEnhancement = (op: Operation): { name: string; config: string; icon: ReactNode } => {
    let config = '';

    switch (true) {
        // Face Recovery
        case op.id.startsWith('fr_athens'): {
            config = `${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;
            return { name: 'Face Recovery', config, icon: <Icon option='face_recovery' /> };
        }

        // Upscale
        case op.id.startsWith('up_kyoto'): {
            config = op.options.mode === 'general' ? 'General' : 'Cartoon';
            config += `, ${op.options.scale}x`;
            config += `, ${op.options.precision === 'fp32' ? 'High' : 'Std.'} Quality`;
            return { name: 'Upscale', config, icon: <Icon option='upscale' /> };
        }
    }

    return { name: '', config: '', icon: null };
};
