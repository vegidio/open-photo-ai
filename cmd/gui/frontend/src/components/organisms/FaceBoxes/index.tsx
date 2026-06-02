import type { File } from '@/bindings/gui/types';
import { useFileFaces } from '@/hooks';

type FaceBoxesProps = {
    file: File;
    // On-screen size of the displayed image.
    displayWidth: number;
    displayHeight: number;
    // True (unscaled) dimensions of the original image the bounding boxes are expressed in.
    originalWidth: number;
    originalHeight: number;
    // Indices of the faces currently disabled (gray); the rest are enabled (yellow).
    disabled: ReadonlySet<number>;
    onToggle: (index: number) => void;
};

export const FaceBoxes = ({
    file,
    displayWidth,
    displayHeight,
    originalWidth,
    originalHeight,
    disabled,
    onToggle,
}: FaceBoxesProps) => {
    const faces = useFileFaces(file);

    if (originalWidth <= 0 || originalHeight <= 0) return null;

    // The bounding boxes are in original-image pixels; scale them to the displayed image size.
    const scaleX = displayWidth / originalWidth;
    const scaleY = displayHeight / originalHeight;

    return (
        <div className='absolute inset-0'>
            {faces.map((face, i) => {
                const { Min, Max } = face.BoundingBox;
                const isDisabled = disabled.has(i);

                return (
                    <button
                        key={i}
                        type='button'
                        aria-label={`Toggle face ${i + 1}`}
                        className={`absolute box-border cursor-pointer appearance-none border-[3px] rounded-md bg-transparent p-0 ${
                            isDisabled ? 'border-gray-500' : 'border-yellow-400'
                        }`}
                        style={{
                            left: Min.X * scaleX,
                            top: Min.Y * scaleY,
                            width: (Max.X - Min.X) * scaleX,
                            height: (Max.Y - Min.Y) * scaleY,
                        }}
                        onClick={() => onToggle(i)}
                    />
                );
            })}
        </div>
    );
};
