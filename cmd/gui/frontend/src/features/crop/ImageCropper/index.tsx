import { forwardRef } from 'react';
import { Cropper, type CropperRef, type CropperState } from 'react-advanced-cropper';
import 'react-advanced-cropper/dist/style.css';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useImageStore } from '@/stores';

type ImageCropperProps = TailwindProps & {
    aspectRatio?: number;
    onChange?: (cropper: CropperRef) => void;
};

const defaultSize = ({ imageSize }: CropperState) => ({
    width: imageSize.width,
    height: imageSize.height,
});

export const ImageCropper = forwardRef<CropperRef, ImageCropperProps>(({ aspectRatio, onChange, className }, ref) => {
    const originalImage = useImageStore((state) => state.originalImage);

    if (!originalImage) return null;

    return (
        <Cropper
            ref={ref}
            src={originalImage.url}
            defaultSize={defaultSize}
            onChange={onChange}
            stencilProps={{
                grid: true,
                aspectRatio,
                overlayClassName: 'text-transparent!',
                gridClassName: 'opacity-100!',
            }}
            className={`size-full bg-transparent! object-contain p-1.5! ${className}`}
        />
    );
});
