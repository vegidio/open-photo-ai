import { useCallback, useEffect, useRef, useState } from 'react';
import { type ReactZoomPanPinchRef, TransformComponent, TransformWrapper } from 'react-zoom-pan-pinch';
import type { ImageData } from '@/utils/image.ts';
import { type ImageTransform, useImageStore } from '@/stores';

type ZoomImageProps = {
    image: ImageData;
    imageTransform: ImageTransform;
};

export const ZoomImage = ({ image, imageTransform }: ZoomImageProps) => {
    const tRef = useRef<ReactZoomPanPinchRef>(null);
    const setImageTransform = useImageStore((state) => state.setImageTransform);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

    const onPanning = (ref: ReactZoomPanPinchRef) => {
        const { positionX, positionY, scale } = ref.state;
        setImageTransform(image.id, { positionX, positionY, scale });
    };

    // Calculate a new position keeping center as the focal point
    const calcPosition = useCallback(
        (center: number, currentPos: number, scaledSize: number, scaleDiff: number, containerSize: number) => {
            const newPos = center - (center - currentPos) * scaleDiff;
            if (scaledSize > containerSize) return Math.max(containerSize - scaledSize, Math.min(0, newPos));
            return (containerSize - scaledSize) / 2;
        },
        [],
    );

    // Set the image dimensions when the image loads
    useEffect(() => {
        const container = tRef.current?.instance.wrapperComponent;
        if (!container) return;

        const rect = container.getBoundingClientRect();
        const scaleX = rect.width / image.width;
        const scaleY = rect.height / image.height;
        const scale = Math.min(scaleX, scaleY);

        setDimensions({
            width: image.width * scale,
            height: image.height * scale,
        });
    }, [image]);

    // Update the zoom level and position of the image
    useEffect(() => {
        if (!tRef.current) return;

        const {
            scale: currentScale,
            positionX: currentPosX,
            positionY: currentPosY,
        } = tRef.current.instance.transformState;
        const { scale: newScale, positionX, positionY } = imageTransform;

        // If scale didn't change, just update the position
        if (currentScale === newScale) {
            tRef.current.setTransform(positionX, positionY, newScale, 0);
            return;
        }

        const container = tRef.current.instance.wrapperComponent;
        if (!container) return;

        const { width: containerWidth, height: containerHeight } = container.getBoundingClientRect();
        const scaleDiff = newScale / currentScale;
        const scaledWidth = dimensions.width * newScale;
        const scaledHeight = dimensions.height * newScale;
        const newPosX = calcPosition(containerWidth / 2, currentPosX, scaledWidth, scaleDiff, containerWidth);
        const newPosY = calcPosition(containerHeight / 2, currentPosY, scaledHeight, scaleDiff, containerHeight);

        tRef.current.setTransform(newPosX, newPosY, newScale, 0);
    }, [imageTransform, dimensions, calcPosition]);

    return (
        <TransformWrapper
            ref={tRef}
            disablePadding={true}
            panning={{ velocityDisabled: true }}
            onPanning={onPanning}
            onPanningStop={onPanning}
            alignmentAnimation={{ animationTime: 0 }}
            doubleClick={{ disabled: true }}
            wheel={{ disabled: true }}
        >
            <TransformComponent
                wrapperStyle={{
                    flex: 1,
                    width: '100%',
                    height: '100%',
                }}
            >
                <img
                    alt='Preview'
                    src={image.url}
                    style={{
                        width: dimensions.width || 'auto',
                        height: dimensions.height || 'auto',
                    }}
                    className='max-w-full max-h-full block'
                />
            </TransformComponent>
        </TransformWrapper>
    );
};
