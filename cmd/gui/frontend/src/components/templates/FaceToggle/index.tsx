import { useEffect, useState } from 'react';
import { Dialog } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { useImageStore } from '@/stores';

type FaceToggleProps = {
    open: boolean;
    onClose: () => void;
};

// ModalTitle header (h-10 = 40px) + 1px Divider.
const TITLE = 41;
// Outer breathing room so the modal never touches the viewport edges.
const MARGIN = 32;

export const FaceToggle = ({ open, onClose }: FaceToggleProps) => {
    const originalImage = useImageStore((state) => state.originalImage);
    const [viewport, setViewport] = useState({ w: window.innerWidth, h: window.innerHeight });

    useEffect(() => {
        const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
        window.addEventListener('resize', onResize);
        return () => window.removeEventListener('resize', onResize);
    }, []);

    if (!originalImage) return null;

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
                if (reason !== 'backdropClick') onClose();
            }}
            slotProps={{
                paper: {
                    className: 'bg-[#212121] max-w-none max-h-none m-0 bg-none flex flex-col',
                    style: { width: imageW, height: imageH + TITLE },
                },
            }}
        >
            <ModalTitle title='Select faces' onClose={onClose} />

            <div className='flex-1 overflow-hidden'>
                <img alt='Original' src={originalImage.url} className='w-full h-full object-contain' />
            </div>
        </Dialog>
    );
};
