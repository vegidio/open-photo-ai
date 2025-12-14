import { IconButton } from '@mui/material';
import { GoFoldDown, GoFoldUp } from 'react-icons/go';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useDrawerStore } from '@/stores';

export const DrawerToggle = ({ className }: TailwindProps) => {
    const open = useDrawerStore((state) => state.open);
    const toggle = useDrawerStore((state) => state.toggle);

    return (
        <IconButton type='button' disableRipple onClick={toggle} className={`${className}`}>
            {open ? <GoFoldDown className='size-6.5' /> : <GoFoldUp className='size-6.5' />}
        </IconButton>
    );
};
