import { IconButton } from '@mui/material';
import { GoFoldDown, GoFoldUp } from 'react-icons/go';
import { useFileListStore } from '@/stores';

export const FileListButton = () => {
    const open = useFileListStore((state) => state.open);
    const toggle = useFileListStore((state) => state.toggle);

    return (
        <IconButton type='button' disableRipple onClick={toggle}>
            {open ? <GoFoldDown className='size-7' /> : <GoFoldUp className='size-7' />}
        </IconButton>
    );
};
