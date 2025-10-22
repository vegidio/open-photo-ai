import { Button, Typography } from '@mui/material';
import { MdFolderOpen } from 'react-icons/md';
import { DialogService } from '../../../bindings/gui/services';
import { useFileStore } from '@/stores';

export const PreviewEmpty = () => {
    const addFilePaths = useFileStore((state) => state.addFilePaths);

    const onBrowseClick = async () => {
        try {
            const paths = await DialogService.OpenFileDialog();
            addFilePaths(paths);
        } catch (e) {
            console.error(e);
        }
    };

    return (
        <div className='flex flex-col items-center justify-center'>
            <MdFolderOpen className='size-20 text-[#009aff]' />

            <div className='flex flex-col text-center gap-3 mb-4 bg-[#171717]'>
                <Typography className='text-[#f2f2f2]'>
                    Drag and drop images or
                    <br />
                    folder to start editing
                </Typography>

                <Typography variant='subtitle2' className='text-[#979797]'>
                    OR
                </Typography>
            </div>

            <Button variant='contained' onClick={onBrowseClick}>
                Browse images
            </Button>
        </div>
    );
};
