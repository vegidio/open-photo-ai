import { Box, CircularProgress } from '@mui/material';
import { MdCloudDownload } from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export const DownloadLoading = ({ className = '' }: TailwindProps) => {
    return (
        <Box className={`${className} relative flex items-center justify-center w-fit`}>
            <CircularProgress variant='indeterminate' size={60} />
            <MdCloudDownload className='absolute size-7' />
        </Box>
    );
};
