import { Slider } from '@mui/material';
import { MdZoomIn, MdZoomOut } from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export const FileListZoom = ({ className }: TailwindProps) => {
    return (
        <div className={`flex flex-row items-center gap-3 ${className}`}>
            <MdZoomOut className='size-5 shrink-0' />
            <Slider size='small' min={50} max={800} defaultValue={100} valueLabelDisplay='auto' />
            <MdZoomIn className='size-5 shrink-0' />
        </div>
    );
};
