import { Button } from '@mui/material';
import { FiPlus } from 'react-icons/fi';
import { DialogService } from '../../../bindings/gui/services';
import { useFileStore } from '@/stores';

export const FileListAddImages = () => {
    const addFiles = useFileStore((state) => state.addFiles);

    const onBrowseClick = async () => {
        try {
            const files = await DialogService.OpenFileDialog();
            addFiles(files);
        } catch (e) {
            console.error(e);
        }
    };

    return (
        <Button
            variant='text'
            disableRipple
            startIcon={<FiPlus className='size-6 stroke-1' />}
            className='normal-case text-white font-normal'
            sx={{
                '&:hover': {
                    backgroundColor: 'transparent',
                },
            }}
            onClick={onBrowseClick}
        >
            Add images
        </Button>
    );
};
