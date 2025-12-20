import { IconButton, Slider } from '@mui/material';
import { FiMinus, FiPlus } from 'react-icons/fi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useFileStore, useImageStore } from '@/stores';

export const DrawerZoom = ({ className }: TailwindProps) => {
    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const imageTransform = useImageStore((state) => state.imageTransform.get(currentFile?.Hash ?? ''));
    const setImageTransform = useImageStore((state) => state.setImageTransform);

    const onSliderChange = (_: Event, value: number) => {
        if (!currentFile) return;

        setImageTransform(currentFile.Hash, {
            scale: value,
            positionX: imageTransform?.positionX ?? 0,
            positionY: imageTransform?.positionY ?? 0,
        });
    };

    const stepZoom = (op: 'plus' | 'minus') => {
        if (!currentFile) return;

        const initialScale = imageTransform?.scale ?? 1;
        const value = op === 'plus' ? Math.min(initialScale + 0.5, 8) : Math.max(initialScale - 0.5, 1);
        if (value === initialScale) return;

        setImageTransform(currentFile.Hash, {
            scale: value,
            positionX: imageTransform?.positionX ?? 0,
            positionY: imageTransform?.positionY ?? 0,
        });
    };

    const valueLabelFormat = (value: number) => {
        return `${value}x`;
    };

    return (
        <div className={`flex flex-row items-center gap-3 ${className}`}>
            <IconButton
                type='button'
                disableRipple
                size='small'
                disabled={!currentFile}
                onClick={() => stepZoom('minus')}
                className='p-0.5'
            >
                <FiMinus className='size-5 stroke-1' />
            </IconButton>

            <Slider
                size='small'
                min={1}
                max={8}
                step={0.1}
                shiftStep={0.1}
                defaultValue={1}
                value={imageTransform?.scale ?? 1}
                valueLabelDisplay='auto'
                valueLabelFormat={valueLabelFormat}
                onChange={onSliderChange}
                disabled={!currentFile}
            />

            <IconButton
                type='button'
                disableRipple
                size='small'
                disabled={!currentFile}
                onClick={() => stepZoom('plus')}
                className='p-0.5'
            >
                <FiPlus className='size-5 stroke-1' />
            </IconButton>
        </div>
    );
};
