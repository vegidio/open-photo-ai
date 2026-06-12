import { IconButton } from '@mui/material';
import { GoFoldDown, GoFoldUp } from 'react-icons/go';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useDrawerStore } from '@/stores';

type DrawerToggleProps = TailwindProps & {
    disabled?: boolean;
};

export const DrawerToggle = ({ disabled = false, className = '' }: DrawerToggleProps) => {
    const open = useDrawerStore((state) => state.open);
    const toggle = useDrawerStore((state) => state.toggle);

    return (
        <IconButton type='button' disableRipple disabled={disabled} onClick={toggle} className={`${className}`}>
            {open ? <GoFoldDown className='size-6.5' /> : <GoFoldUp className='size-6.5' />}
        </IconButton>
    );
};
