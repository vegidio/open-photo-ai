import { useEffect, useRef, useState } from 'react';
import { Dialog } from '@mui/material';
import type { CropperRef } from 'react-advanced-cropper';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { AspectRatioSelector } from '@/features/crop/AspectRatioSelector';
import { ImageCropper } from '@/features/crop/ImageCropper';
import { RotateControls } from '@/features/crop/RotateControls';
import { useImageStore } from '@/stores';
import { DOTTED_BACKGROUND, MIN_CROP_SIZE } from '@/utils/constants.ts';

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
    const [cropWidth, setCropWidth] = useState(0);
    const [cropHeight, setCropHeight] = useState(0);
    const [pendingSwap, setPendingSwap] = useState<{ width: number; height: number } | null>(null);

    useEffect(() => {
        if (open) {
            setRatio('free');
            setAspectRatio(undefined);
            setBaseRotation(0);
            setFineRotation(0);
            setCropWidth(originalImage?.width ?? 0);
            setCropHeight(originalImage?.height ?? 0);
        }
    }, [open, originalImage]);

    // Apply a swapped crop only after the cleared aspect ratio has propagated to the <Cropper> stencil; doing it
    // synchronously in onSwap would re-constrain the box to the previous ratio (same deferral as pendingReset).
    useEffect(() => {
        if (!pendingSwap) return;
        cropperRef.current?.setCoordinates((state) => ({
            left: state.coordinates?.left ?? 0,
            top: state.coordinates?.top ?? 0,
            width: pendingSwap.width,
            height: pendingSwap.height,
        }));
        setPendingSwap(null);
    }, [pendingSwap]);

    // Reshape the crop box to a newly selected aspect ratio and sync the W/H fields. The work is deferred to the next
    // frame because on the render that sets the ratio, the <Cropper> instance may not be mounted yet, and the new
    // aspectRatio still needs to reach the stencil. By then we resize the box to the largest box of that ratio that
    // fits inside the current box (centered) and push the applied size into the fields directly — the cropper
    // does not fire onChange for a ratio-driven reshape, so the fields would not update otherwise. 'free' is a no-op.
    useEffect(() => {
        if (!open || aspectRatio === undefined) return;
        const frame = requestAnimationFrame(() => {
            const cropper = cropperRef.current;
            if (!cropper) return;

            cropper.setCoordinates((state) => {
                const coords = state.coordinates ?? {
                    left: 0,
                    top: 0,
                    width: state.imageSize.width,
                    height: state.imageSize.height,
                };
                const centerX = coords.left + coords.width / 2;
                const centerY = coords.top + coords.height / 2;
                const width = coords.width / coords.height > aspectRatio ? coords.height * aspectRatio : coords.width;
                const height = width / aspectRatio;

                return { width, height, left: centerX - width / 2, top: centerY - height / 2 };
            });

            const applied = cropper.getCoordinates({ round: true });
            if (applied) {
                setCropWidth(applied.width);
                setCropHeight(applied.height);
            }
        });

        return () => cancelAnimationFrame(frame);
    }, [aspectRatio, open]);

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

    const clampWidth = (value: number) => Math.max(MIN_CROP_SIZE, Math.min(originalImage.width, Math.round(value)));
    const clampHeight = (value: number) => Math.max(MIN_CROP_SIZE, Math.min(originalImage.height, Math.round(value)));

    // Mirror the live stencil size into the W/H fields on every drag/resize and after programmatic changes.
    const syncDims = (cropper: CropperRef) => {
        const coords = cropper.getCoordinates({ round: true });
        if (!coords) return;
        setCropWidth(coords.width);
        setCropHeight(coords.height);
    };

    const applyDimensions = (width: number, height: number) => {
        cropperRef.current?.setCoordinates((state) => ({
            left: state.coordinates?.left ?? 0,
            top: state.coordinates?.top ?? 0,
            width,
            height,
        }));
    };

    const onWidthCommit = (value: number) => {
        const width = clampWidth(value);
        const height = aspectRatio
            ? clampHeight(width / aspectRatio)
            : (cropperRef.current?.getCoordinates()?.height ?? width);
        applyDimensions(width, height);
    };

    const onHeightCommit = (value: number) => {
        const height = clampHeight(value);
        const width = aspectRatio
            ? clampWidth(height * aspectRatio)
            : (cropperRef.current?.getCoordinates()?.width ?? height);
        applyDimensions(width, height);
    };

    // Swap W↔H and drop any locked ratio; the actual resize is deferred to the pendingSwap effect, so it runs after
    // the cleared aspect ratio reaches the stencil.
    const onSwap = () => {
        const coords = cropperRef.current?.getCoordinates();
        if (!coords) return;
        setRatio('free');
        setAspectRatio(undefined);
        setPendingSwap({ width: clampWidth(coords.height), height: clampHeight(coords.width) });
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
                    <ImageCropper ref={cropperRef} aspectRatio={aspectRatio} onChange={syncDims} />

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
                <AspectRatioSelector
                    selected={ratio}
                    onSelect={(key, value) => {
                        setRatio(key);
                        setAspectRatio(value);
                    }}
                    width={String(cropWidth)}
                    height={String(cropHeight)}
                    onWidthCommit={onWidthCommit}
                    onHeightCommit={onHeightCommit}
                    onSwap={onSwap}
                />
            </div>
        </Dialog>
    );
};
