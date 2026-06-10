import { useEffect, useRef, useState } from 'react';
import { Dialog } from '@mui/material';
import type { CropperRef } from 'react-advanced-cropper';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { RatioSelector } from '@/components/molecules/RatioSelector';
import { RotateControls } from '@/components/molecules/RotateControls';
import { ImageCropper } from '@/components/organisms/ImageCropper';
import { useImageStore } from '@/stores';
import { DOTTED_BACKGROUND } from '@/utils/constants.ts';

type CropRotateProps = {
    open: boolean;
    onClose: () => void;
};

export const CropRotate = ({ open, onClose }: CropRotateProps) => {
    const originalImage = useImageStore((state) => state.originalImage);
    const cropperRef = useRef<CropperRef>(null);
    const [ratio, setRatio] = useState('free');
    const [aspectRatio, setAspectRatio] = useState<number | undefined>(undefined);
    const [baseRotation, setBaseRotation] = useState(0);
    const [fineRotation, setFineRotation] = useState(0);
    const [pendingReset, setPendingReset] = useState(false);

    useEffect(() => {
        if (open) {
            setRatio('free');
            setAspectRatio(undefined);
            setBaseRotation(0);
            setFineRotation(0);
        }
    }, [open]);

    // Reset the crop coordinates to the full image only after the cleared aspect ratio has propagated to the
    // <Cropper> stencil; doing it synchronously in onReset would re-constrain the box to the previous ratio.
    useEffect(() => {
        if (!pendingReset) return;
        cropperRef.current?.setCoordinates((state) => ({
            left: 0,
            top: 0,
            width: state.imageSize.width,
            height: state.imageSize.height,
        }));
        setPendingReset(false);
    }, [pendingReset]);

    if (!originalImage) return null;

    // Rotate the cropper image to the given absolute angle by applying the delta from its current rotation.
    // The delta is normalized to the shortest path (-180, 180) so equivalent angles (e.g. 0 vs. 360) produce no
    // movement and a reset never spins multiple full turns.
    const applyRotation = (target: number, immediate = false) => {
        const current = cropperRef.current?.getState()?.transforms.rotate ?? 0;
        let delta = (((target - current) % 360) + 360) % 360; // [0, 360)
        if (delta > 180) delta -= 360; // (-180, 180]
        cropperRef.current?.rotateImage(delta, immediate ? { transitions: false } : undefined);
    };

    const onRotationChange = (value: number) => {
        setFineRotation(value);
        applyRotation(baseRotation + value, true); // continuous drag → no animation, smooth tracking
    };

    const onRotate90 = () => {
        const next = (Math.floor((baseRotation + fineRotation) / 90) + 1) * 90;
        setBaseRotation(next % 360);
        setFineRotation(0);
        applyRotation(next);
    };

    const onFlipHorizontal = () => cropperRef.current?.flipImage(true, false);

    const onFlipVertical = () => cropperRef.current?.flipImage(false, true);

    const onReset = () => {
        setRatio('free');
        setAspectRatio(undefined);
        setBaseRotation(0);
        setFineRotation(0);
        applyRotation(0);

        // Toggle any active flips back to the original orientation.
        const flip = cropperRef.current?.getState()?.transforms.flip;
        if (flip?.horizontal || flip?.vertical) {
            cropperRef.current?.flipImage(flip.horizontal, flip.vertical);
        }

        setPendingReset(true);
    };

    return (
        <Dialog
            open={open}
            onClose={(_, reason) => {
                if (reason !== 'backdropClick') onClose();
            }}
            slotProps={{
                paper: {
                    className:
                        'bg-[#212121] bg-none max-w-none max-h-none m-8 w-[calc(100vw-64px)] h-[calc(100vh-64px)] flex flex-col',
                },
            }}
        >
            <ModalTitle title='Crop/Rotate' onClose={onClose} />

            <div className={`flex-1 flex flex-row overflow-hidden ${DOTTED_BACKGROUND}`}>
                {/* Left */}
                <div className='flex-1 flex flex-col overflow-hidden p-8 gap-4'>
                    <ImageCropper ref={cropperRef} aspectRatio={aspectRatio} />

                    <RotateControls
                        rotation={fineRotation}
                        onRotationChange={onRotationChange}
                        onRotate90={onRotate90}
                        onFlipHorizontal={onFlipHorizontal}
                        onFlipVertical={onFlipVertical}
                        onReset={onReset}
                        className='w-1/2 self-center'
                    />
                </div>

                {/* Right */}
                <RatioSelector
                    selected={ratio}
                    onSelect={(key, value) => {
                        setRatio(key);
                        setAspectRatio(value);
                    }}
                />
            </div>
        </Dialog>
    );
};
