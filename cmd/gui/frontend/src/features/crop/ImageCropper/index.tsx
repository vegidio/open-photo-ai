import { forwardRef } from 'react';
import { Cropper, type CropperRef, type CropperState } from 'react-advanced-cropper';
import 'react-advanced-cropper/dist/style.css';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type ImageCropperProps = TailwindProps & {
    src: string;
    aspectRatio?: number;
    onChange?: (cropper: CropperRef) => void;
    onReady?: (cropper: CropperRef) => void;
};

const defaultSize = ({ imageSize }: CropperState) => ({
    width: imageSize.width,
    height: imageSize.height,
});

export const ImageCropper = forwardRef<CropperRef, ImageCropperProps>(
    ({ src, aspectRatio, onChange, onReady, className }, ref) => {
        return (
            <Cropper
                ref={ref}
                src={src}
                defaultSize={defaultSize}
                onChange={onChange}
                onReady={onReady}
                stencilProps={{
                    grid: true,
                    aspectRatio,
                    overlayClassName: 'text-transparent!',
                    gridClassName: 'opacity-100!',
                }}
                className={`size-full bg-transparent! object-contain p-1.5! ${className}`}
            />
        );
    },
);
