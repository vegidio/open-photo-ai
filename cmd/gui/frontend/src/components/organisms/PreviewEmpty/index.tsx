import { Typography } from '@mui/material';
import { MdFolderOpen } from 'react-icons/md';
import { DialogService } from '@/bindings/gui/services';
import { Button } from '@/components/atoms/Button';
import { useFileStore } from '@/stores';

export const PreviewEmpty = () => {
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
        <div className='flex flex-col items-center justify-center size-full'>
            <MdFolderOpen className='size-20 text-[#009aff]' />

            <div className='flex flex-col text-center gap-3 mb-4 bg-[#171717]'>
                <Typography className='text-[#f2f2f2]'>
                    Drag and drop images
                    <br />
                    to start editing them
                </Typography>

                <Typography variant='subtitle2' className='text-[#979797]'>
                    OR
                </Typography>
            </div>

            <Button onClick={onBrowseClick}>Browse images</Button>
        </div>
    );
};
