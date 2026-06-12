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
    const setViewport = useImageStore((state) => state.setViewport);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

    const onPanning = (ref: ReactZoomPanPinchRef) => {
        const { positionX, positionY, scale } = ref.state;
        setImageTransform(image.id, { positionX, positionY, scale });
    };

    // Center image if smaller than container, otherwise constrain within bounds
    const constrainPosition = useCallback((position: number, scaledSize: number, containerSize: number) => {
        if (scaledSize <= containerSize) return (containerSize - scaledSize) / 2;
        return Math.max(containerSize - scaledSize, Math.min(0, position));
    }, []);

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
        const container = tRef.current?.instance.wrapperComponent;
        if (!tRef.current || !container) return;

        const { width: containerWidth, height: containerHeight } = container.getBoundingClientRect();
        const {
            scale: currentScale,
            positionX: currentPosX,
            positionY: currentPosY,
        } = tRef.current.instance.transformState;
        const { scale: newScale, positionX, positionY } = imageTransform;
        const scaledWidth = dimensions.width * newScale;
        const scaledHeight = dimensions.height * newScale;

        // Calculate new positions based on whether the scale changed
        let newPosX: number, newPosY: number;

        // If scale didn't change, just update the position
        if (currentScale === newScale) {
            newPosX = constrainPosition(positionX, scaledWidth, containerWidth);
            newPosY = constrainPosition(positionY, scaledHeight, containerHeight);
        } else {
            const scaleDiff = newScale / currentScale;
            const centerX = containerWidth / 2;
            const centerY = containerHeight / 2;
            newPosX = constrainPosition(centerX - (centerX - currentPosX) * scaleDiff, scaledWidth, containerWidth);
            newPosY = constrainPosition(centerY - (centerY - currentPosY) * scaleDiff, scaledHeight, containerHeight);
        }

        tRef.current.setTransform(newPosX, newPosY, newScale, 0);

        // Publish the portion of the image visible in this pane as fractions [0..1] of the displayed image.
        // The container is each pane's own wrapper, so this is already half-width in "side" mode.
        const visLeft = Math.max(0, newPosX);
        const visTop = Math.max(0, newPosY);
        const visRight = Math.min(containerWidth, newPosX + scaledWidth);
        const visBottom = Math.min(containerHeight, newPosY + scaledHeight);

        setViewport(
            scaledWidth && scaledHeight
                ? {
                      x: (visLeft - newPosX) / scaledWidth,
                      y: (visTop - newPosY) / scaledHeight,
                      width: Math.max(0, visRight - visLeft) / scaledWidth,
                      height: Math.max(0, visBottom - visTop) / scaledHeight,
                  }
                : { x: 0, y: 0, width: 1, height: 1 },
        );
    }, [imageTransform, dimensions, constrainPosition, setViewport]);

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
                contentStyle={{
                    cursor: 'grab',
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
