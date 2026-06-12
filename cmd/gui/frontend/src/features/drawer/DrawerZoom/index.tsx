import { IconButton, Slider } from '@mui/material';
import { FiMinus, FiPlus } from 'react-icons/fi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useCurrentFile, useImageTransform } from '@/hooks';
import { useImageStore } from '@/stores';
import { ZOOM_MAX, ZOOM_MIN } from '@/utils/constants.ts';

type DrawerZoomProps = TailwindProps & {
    disabled?: boolean;
};

export const DrawerZoom = ({ disabled = false, className = '' }: DrawerZoomProps) => {
    const currentFile = useCurrentFile();
    const imageTransform = useImageTransform(currentFile);
    const setImageTransform = useImageStore((state) => state.setImageTransform);

    const onSliderChange = (_: Event, value: number) => {
        if (!currentFile) return;

        setImageTransform(currentFile.Hash, {
            scale: value,
            positionX: imageTransform.positionX,
            positionY: imageTransform.positionY,
        });
    };

    const stepZoom = (op: 'plus' | 'minus') => {
        if (!currentFile) return;

        const initialScale = imageTransform.scale;
        const value = op === 'plus' ? Math.min(initialScale + 0.5, ZOOM_MAX) : Math.max(initialScale - 0.5, ZOOM_MIN);
        if (value === initialScale) return;

        setImageTransform(currentFile.Hash, {
            scale: value,
            positionX: imageTransform.positionX,
            positionY: imageTransform.positionY,
        });
    };

    const valueLabelFormat = (value: number) => {
        return `${value.toFixed(1)}x`;
    };

    return (
        <div className={`flex flex-row items-center gap-3 ${className}`}>
            <IconButton
                type='button'
                disableRipple
                size='small'
                disabled={disabled}
                onClick={() => stepZoom('minus')}
                className='p-0.5'
            >
                <FiMinus className='size-5 stroke-1' />
            </IconButton>

            <Slider
                size='small'
                min={ZOOM_MIN}
                max={ZOOM_MAX}
                step={0.1}
                shiftStep={0.1}
                defaultValue={1}
                value={imageTransform.scale}
                valueLabelDisplay='auto'
                valueLabelFormat={valueLabelFormat}
                onChange={onSliderChange}
                disabled={disabled}
            />

            <IconButton
                type='button'
                disableRipple
                size='small'
                disabled={disabled}
                onClick={() => stepZoom('plus')}
                className='p-0.5'
            >
                <FiPlus className='size-5 stroke-1' />
            </IconButton>
        </div>
    );
};
