import { IconButton } from '@mui/material';
import { GoFoldDown, GoFoldUp } from 'react-icons/go';
import { useDrawerStore } from '@/stores';

export const DrawerToggle = () => {
    const open = useDrawerStore((state) => state.open);
    const toggle = useDrawerStore((state) => state.toggle);

    return (
        <IconButton type='button' disableRipple onClick={toggle}>
            {open ? <GoFoldDown className='size-7' /> : <GoFoldUp className='size-7' />}
        </IconButton>
    );
};
