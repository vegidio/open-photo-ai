import type { ReactNode } from 'react';
import { IconButton, ListItem, ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import { MdClose, MdOpenInFull, MdOutlineFaceRetouchingNatural } from 'react-icons/md';
import type { Operation } from '@/stores';

type EnhancementProps = {
    op: Operation;
};

export const Enhancement = ({ op }: EnhancementProps) => {
    const { name, config, icon } = opToEnhancement(op);

    return (
        <ListItem
            disablePadding
            secondaryAction={
                <IconButton edge='end'>
                    <MdClose />
                </IconButton>
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

    switch (op.id) {
        case 'face': {
            config = `${op.options.precision === 'fp32' ? 'High' : 'Medium'} Quality`;
            return { name: 'Face Recovery', config, icon: <MdOutlineFaceRetouchingNatural /> };
        }

        case 'upscale': {
            config = op.options.mode === 'general' ? 'General' : 'Cartoon';
            config += `, ${op.options.scale}x`;
            config += `, ${op.options.precision === 'fp32' ? 'High' : 'Medium'} Quality`;
            return { name: 'Upscale', config, icon: <MdOpenInFull /> };
        }
    }

    return { name: '', config: '', icon: null };
};
