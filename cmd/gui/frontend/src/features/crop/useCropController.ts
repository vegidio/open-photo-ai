import { useEffect, useRef, useState } from 'react';
import type { CropperRef } from 'react-advanced-cropper';
import { CropInfo } from '@/bindings/gui/types';
import { FULL_TURN, normalizeAngle, ROTATE_STEP, snapToStep } from '@/features/crop/utils.ts';
import { useCurrentFile, useFileCrop } from '@/hooks';
import { useCropStore } from '@/stores';
import { MIN_CROP_SIZE } from '@/utils/constants.ts';
import { getImage, type ImageData } from '@/utils/image.ts';

// Owns all the Crop/Rotate modal's state, effects, and cropper-control handlers, keeping CropRotate itself a thin
// layout component. The handlers reference `baseImage`/`currentFile` defensively because they're defined before the
// image has loaded, but the modal only wires them to controls once `baseImage` is truthy.
export const useCropController = (open: boolean, onClose: () => void) => {
    const currentFile = useCurrentFile();
    const savedCrop = useFileCrop(currentFile);
    const setCrop = useCropStore((state) => state.setCrop);
    const removeCrop = useCropStore((state) => state.removeKey);

    const cropperRef = useRef<CropperRef>(null);

    // The Crop/Rotate modal always edits the *uncropped* original (the store's originalImage may already be cropped),
    // so it fetches its own size=0 copy without any crop applied.
    const [baseImage, setBaseImage] = useState<ImageData>();
    const [ratio, setRatio] = useState('free');
    const [aspectRatio, setAspectRatio] = useState<number>();
    const [baseRotation, setBaseRotation] = useState(0);
    const [fineRotation, setFineRotation] = useState(0);
    const [pendingReset, setPendingReset] = useState(false);
    const [cropWidth, setCropWidth] = useState(0);
    const [cropHeight, setCropHeight] = useState(0);
    const [pendingSwap, setPendingSwap] = useState<{ width: number; height: number }>();

    // Load the uncropped original whenever the modal opens for a file.
    useEffect(() => {
        if (!open || !currentFile) return;
        let cancelled = false;

        getImage(currentFile, 0)
            .then((img) => {
                if (!cancelled) setBaseImage(img);
            })
            .catch((e) => console.error('Failed to load image for Crop/Rotate', e));

        return () => {
            cancelled = true;
        };
    }, [open, currentFile]);

    // Reset the UI controls to defaults when (re)opening; the cropper itself is seeded from any saved crop in onReady.
    useEffect(() => {
        if (open && baseImage) {
            setRatio('free');
            setAspectRatio(undefined);
            setBaseRotation(0);
            setFineRotation(0);
            setCropWidth(baseImage.width);
            setCropHeight(baseImage.height);
        }
    }, [open, baseImage]);

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

        setPendingSwap(undefined);
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

    // Seed the cropper from a previously saved crop once the image is loaded, so re-opening shows the uncropped
    // original with the prior flip/rotate/crop applied (and therefore editable/revertible).
    const onReady = (cropper: CropperRef) => {
        if (!savedCrop) return;
        if (savedCrop.FlipH || savedCrop.FlipV) cropper.flipImage(savedCrop.FlipH, savedCrop.FlipV);
        if (savedCrop.Rotation !== 0) cropper.rotateImage(savedCrop.Rotation, { transitions: false });

        cropper.setCoordinates({
            left: savedCrop.Left,
            top: savedCrop.Top,
            width: savedCrop.Width,
            height: savedCrop.Height,
        });

        const base = snapToStep(savedCrop.Rotation);
        setBaseRotation(base);
        setFineRotation(savedCrop.Rotation - base);
        setCropWidth(savedCrop.Width);
        setCropHeight(savedCrop.Height);
    };

    // Rotate the cropper image to the given absolute angle by applying the delta from its current rotation.
    // The delta is normalized to the shortest path (-180, 180) so equivalent angles (e.g. 0 vs. 360) produce no
    // movement and a reset never spins multiple full turns.
    const applyRotation = (target: number, immediate = false) => {
        const current = cropperRef.current?.getState()?.transforms.rotate ?? 0;
        let delta = normalizeAngle(target - current); // [0, 360)
        if (delta > FULL_TURN / 2) delta -= FULL_TURN; // (-180, 180]
        cropperRef.current?.rotateImage(delta, immediate ? { transitions: false } : undefined);
    };

    const onRotationChange = (value: number) => {
        setFineRotation(value);
        applyRotation(baseRotation + value, true); // continuous drag → no animation, smooth tracking
    };

    const onRotate90 = () => {
        const next = snapToStep(baseRotation + fineRotation) + ROTATE_STEP;
        setBaseRotation(next % FULL_TURN);
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

    const clampWidth = (value: number) =>
        baseImage ? Math.max(MIN_CROP_SIZE, Math.min(baseImage.width, Math.round(value))) : Math.round(value);
    const clampHeight = (value: number) =>
        baseImage ? Math.max(MIN_CROP_SIZE, Math.min(baseImage.height, Math.round(value))) : Math.round(value);

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

    const onSelectRatio = (key: string, value?: number) => {
        setRatio(key);
        setAspectRatio(value);
    };

    const onApply = () => {
        const coords = cropperRef.current?.getCoordinates({ round: true });
        if (!coords || !currentFile || !baseImage) return;

        const rotation = normalizeAngle(baseRotation + fineRotation); // normalize to [0, 360)
        const flip = cropperRef.current?.getState()?.transforms.flip;
        const flipH = flip?.horizontal ?? false;
        const flipV = flip?.vertical ?? false;

        // An unchanged crop (full box, no rotation, no flip) clears any stored crop so the preview reverts cleanly.
        const isIdentity =
            rotation === 0 &&
            !flipH &&
            !flipV &&
            coords.left === 0 &&
            coords.top === 0 &&
            coords.width === baseImage.width &&
            coords.height === baseImage.height;

        if (isIdentity) {
            removeCrop(currentFile);
        } else {
            setCrop(
                currentFile,
                new CropInfo({
                    Rotation: rotation,
                    FlipH: flipH,
                    FlipV: flipV,
                    Top: coords.top,
                    Left: coords.left,
                    Width: coords.width,
                    Height: coords.height,
                }),
            );
        }

        onClose();
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

    return {
        cropperRef,
        baseImage,
        ratio,
        aspectRatio,
        fineRotation,
        cropWidth,
        cropHeight,
        onReady,
        syncDims,
        onRotationChange,
        onRotate90,
        onFlipHorizontal,
        onFlipVertical,
        onReset,
        onSelectRatio,
        onWidthCommit,
        onHeightCommit,
        onSwap,
        onApply,
    };
};
