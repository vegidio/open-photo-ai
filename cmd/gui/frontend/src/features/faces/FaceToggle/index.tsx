import { useEffect, useState } from 'react';
import { Dialog } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { FaceBoxes } from '@/features/faces/FaceBoxes';
import { useFaceSelection } from '@/hooks';
import { useImageStore } from '@/stores';

type FaceToggleProps = {
    file: File;
    open: boolean;
    onClose: () => void;
};

// ModalTitle header (h-10 = 40px) + 1px Divider.
const TITLE = 41;
// Outer breathing room so the modal never touches the viewport edges.
const MARGIN = 32;

export const FaceToggle = ({ file, open, onClose }: FaceToggleProps) => {
    const originalImage = useImageStore((state) => state.originalImage);
    const { disabled, toggle, commit } = useFaceSelection(file, open);
    const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });

    // Commit the working selection to the store only on close, so toggling many faces triggers a single inference.
    const handleClose = () => {
        commit();
        onClose();
    };

    useEffect(() => {
        const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
        window.addEventListener('resize', onResize);
        return () => window.removeEventListener('resize', onResize);
    }, []);

    if (!originalImage) return undefined;

    // Grow the modal to fill the screen while preserving the image aspect ratio, leaving room for the title bar.
    const aspectRatio = originalImage.width / originalImage.height;
    const availW = viewport.w - 2 * MARGIN;
    const availH = viewport.h - 2 * MARGIN;
    const imageH = Math.min(availH - TITLE, availW / aspectRatio);
    const imageW = imageH * aspectRatio;

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') handleClose();
            }}
            slotProps={{
                paper: {
                    className: 'bg-[#212121] max-w-none max-h-none m-0 bg-none flex flex-col',
                    style: { width: imageW, height: imageH + TITLE },
                },
            }}
        >
            <ModalTitle title='Select faces' onClose={handleClose} />

            <div className='flex-1 overflow-hidden relative'>
                <img alt='Original' src={originalImage.url} className='w-full h-full object-contain' />

                <FaceBoxes
                    file={file}
                    displayWidth={imageW}
                    displayHeight={imageH}
                    originalWidth={originalImage.width}
                    originalHeight={originalImage.height}
                    disabled={disabled}
                    onToggle={toggle}
                />
            </div>
        </Dialog>
    );
};
