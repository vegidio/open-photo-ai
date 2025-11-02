import { useEffect, useRef, useState } from 'react';
import { type ReactZoomPanPinchRef, TransformComponent, TransformWrapper } from 'react-zoom-pan-pinch';
import type { ImageData } from '@/utils/image.ts';
import { useImageStore } from '@/stores';

type ZoomImageProps = {
    image: ImageData;
};

export const ZoomImage = ({ image }: ZoomImageProps) => {
    const imageState = useImageStore((state) => state.imageState);
    const setImageState = useImageStore((state) => state.setImageState);

    const tRef = useRef<ReactZoomPanPinchRef>(null);
    const [originalScale, setOriginalScale] = useState(1);

    const onPanning = (ref: ReactZoomPanPinchRef) => {
        const { positionX, positionY } = ref.state;
        const scale = imageState?.scale ?? 1;
        setImageState({ scale, positionX, positionY });
    };

    // Scale the image to fit the container when it's opened
    useEffect(() => {
        const rect = tRef.current?.instance.wrapperComponent?.getBoundingClientRect();
        if (!rect) return;

        const scaleX = rect.width / image.width;
        const scaleY = rect.height / image.height;
        const fitScale = Math.min(scaleX, scaleY) < 1 ? 1 : Math.min(scaleX, scaleY);

        setOriginalScale(fitScale);
    }, [image]);

    // Control the panning & zoom
    // TODO: Fix the panning issue when the image is zoomed out
    useEffect(() => {
        const x = imageState?.positionX ?? 0;
        const y = imageState?.positionY ?? 0;
        const scale = originalScale * (imageState?.scale ?? 1);
        tRef.current?.setTransform(x, y, scale, 0);
    }, [imageState?.positionX, imageState?.positionY, imageState?.scale, originalScale]);

    return (
        <TransformWrapper
            ref={tRef}
            maxScale={8}
            panning={{ velocityDisabled: true }}
            onPanning={onPanning}
            onPanningStop={onPanning}
            alignmentAnimation={{ animationTime: 0 }}
        >
            <TransformComponent wrapperStyle={{ flex: 1, width: '100%', height: '100%' }}>
                <img alt='Preview' src={image.url} />
            </TransformComponent>
        </TransformWrapper>
    );
};
