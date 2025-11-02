import { Slider } from '@mui/material';
import { MdZoomIn, MdZoomOut } from 'react-icons/md';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useImageStore } from '@/stores';

export const FileListZoom = ({ className }: TailwindProps) => {
    const imageState = useImageStore((state) => state.imageState);
    const setImageState = useImageStore((state) => state.setImageState);

    const onSliderChange = (_: Event, value: number) => {
        setImageState({ scale: value, positionX: imageState?.positionX ?? 0, positionY: imageState?.positionY ?? 0 });
    };

    const valueLabelFormat = (value: number) => {
        return `${value}x`;
    };

    return (
        <div className={`flex flex-row items-center gap-3 ${className}`}>
            <MdZoomOut className='size-5 shrink-0' />
            <Slider
                size='small'
                min={1}
                max={8}
                step={0.1}
                shiftStep={0.1}
                defaultValue={1}
                value={imageState?.scale ?? 1}
                valueLabelDisplay='auto'
                valueLabelFormat={valueLabelFormat}
                onChange={onSliderChange}
            />
            <MdZoomIn className='size-5 shrink-0' />
        </div>
    );
};
