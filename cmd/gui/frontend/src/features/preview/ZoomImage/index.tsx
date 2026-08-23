import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { type ReactZoomPanPinchRef, TransformComponent, TransformWrapper } from 'react-zoom-pan-pinch';
import type { ImageData } from '@/utils/image.ts';
import { type ImageTransform, useImageStore } from '@/stores';
import { ZOOM_MAX, ZOOM_MIN, ZOOM_WHEEL_STEP } from '@/utils/constants.ts';

// Position within the displayed image as a fraction [0..1], clamped so points outside the image
// (the letterbox margins) resolve to the nearest edge. Falls back to the middle before measuring.
const imageFraction = (offset: number, scaledSize: number) =>
    scaledSize > 0 ? Math.min(Math.max(offset / scaledSize, 0), 1) : 0.5;

type ZoomImageProps = {
    image: ImageData;
    imageTransform: ImageTransform;
};

export const ZoomImage = ({ image, imageTransform }: ZoomImageProps) => {
    const { t } = useTranslation();
    const tRef = useRef<ReactZoomPanPinchRef>(null);
    const setImageTransform = useImageStore((state) => state.setImageTransform);
    const setViewport = useImageStore((state) => state.setViewport);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

    // Panning fires on every pointer frame, and each store write re-renders both panes in side/split mode (they share
    // one transform key), re-runs the transform effect and republishes the viewport rect - a sustained ~60Hz cascade
    // through the tree for the length of a drag. Coalescing to one write per animation frame keeps the transform in
    // sync without making the store the bottleneck. useCallback because a fresh identity per render would defeat any
    // memoisation downstream.
    const pendingPan = useRef<ImageTransform>(undefined);
    const panFrame = useRef<number>(undefined);

    const onPanning = useCallback(
        (ref: ReactZoomPanPinchRef) => {
            const { positionX, positionY, scale } = ref.state;
            pendingPan.current = { positionX, positionY, scale };

            if (panFrame.current !== undefined) return;

            panFrame.current = requestAnimationFrame(() => {
                panFrame.current = undefined;
                if (pendingPan.current) setImageTransform(image.id, pendingPan.current);
            });
        },
        [image.id, setImageTransform],
    );

    // Flush on unmount, so a drag that ends with the pane closing does not drop its last position.
    useEffect(
        () => () => {
            if (panFrame.current === undefined) return;

            cancelAnimationFrame(panFrame.current);
            panFrame.current = undefined;
        },
        [],
    );

    // Center image if smaller than container, otherwise constrain within bounds
    const constrainPosition = useCallback((position: number, scaledSize: number, containerSize: number) => {
        if (scaledSize <= containerSize) return (containerSize - scaledSize) / 2;
        return Math.max(containerSize - scaledSize, Math.min(0, position));
    }, []);

    // Measured on every container resize, not only when the image changes. Everything downstream - the wheel anchor,
    // constrainPosition, and the viewport rect published to the sidebar - is derived from these numbers, so measuring
    // once left all three working off stale values after a window resize or a sidebar toggle.
    useEffect(() => {
        const container = tRef.current?.instance.wrapperComponent;
        if (!container) return;

        const measure = () => {
            const rect = container.getBoundingClientRect();
            const scale = Math.min(rect.width / image.width, rect.height / image.height);

            setDimensions((current) => {
                const width = image.width * scale;
                const height = image.height * scale;

                // Same numbers means the same object, so a resize that changes nothing does not re-render.
                return current.width === width && current.height === height ? current : { width, height };
            });
        };

        measure();

        if (typeof ResizeObserver === 'undefined') return;

        const observer = new ResizeObserver(measure);
        observer.observe(container);

        return () => observer.disconnect();
    }, [image]);

    // Zoom in/out with the mouse wheel, or a trackpad pinch which the webview delivers as a wheel
    // event, while hovering the preview. We write the clamped scale plus the image point under the
    // cursor, and let the transform effect below re-apply the transform anchored on that point, so
    // the drawer slider stays in sync. A native, non-passive listener is required because React's
    // synthetic onWheel is passive, making preventDefault() a no-op.
    useEffect(() => {
        const container = tRef.current?.instance.wrapperComponent;
        if (!container) return;

        const onWheel = (e: WheelEvent) => {
            e.preventDefault();
            const state = tRef.current?.instance.state;
            if (!state) return;

            const dir = e.deltaY < 0 ? 1 : -1; // wheel up = zoom in
            const next = Math.min(Math.max(state.scale + dir * ZOOM_WHEEL_STEP, ZOOM_MIN), ZOOM_MAX);
            if (next === state.scale) return;

            // Anchor the zoom on the point under the cursor, as a fraction of the displayed image so
            // the sibling pane in "side"/"split" mode can apply it to its own container.
            const rect = container.getBoundingClientRect();
            const anchor = {
                x: imageFraction(e.clientX - rect.left - state.positionX, dimensions.width * state.scale),
                y: imageFraction(e.clientY - rect.top - state.positionY, dimensions.height * state.scale),
            };

            setImageTransform(image.id, {
                scale: next,
                positionX: state.positionX,
                positionY: state.positionY,
                anchor,
            });
        };

        container.addEventListener('wheel', onWheel, { passive: false });
        return () => container.removeEventListener('wheel', onWheel);
    }, [image.id, dimensions, setImageTransform]);

    useEffect(() => {
        const container = tRef.current?.instance.wrapperComponent;
        if (!tRef.current || !container) return;

        const { width: containerWidth, height: containerHeight } = container.getBoundingClientRect();
        const { scale: currentScale, positionX: currentPosX, positionY: currentPosY } = tRef.current.instance.state;
        const { scale: newScale, positionX, positionY, anchor } = imageTransform;
        const scaledWidth = dimensions.width * newScale;
        const scaledHeight = dimensions.height * newScale;

        let newPosX: number, newPosY: number;

        if (currentScale === newScale) {
            newPosX = constrainPosition(positionX, scaledWidth, containerWidth);
            newPosY = constrainPosition(positionY, scaledHeight, containerHeight);
        } else {
            // Keep one point of the image pinned where it already is on screen: the point under the
            // cursor when zooming with the wheel, or the container center for the drawer controls.
            const prevWidth = dimensions.width * currentScale;
            const prevHeight = dimensions.height * currentScale;
            const anchorX = anchor?.x ?? imageFraction(containerWidth / 2 - currentPosX, prevWidth);
            const anchorY = anchor?.y ?? imageFraction(containerHeight / 2 - currentPosY, prevHeight);
            const screenX = currentPosX + anchorX * prevWidth;
            const screenY = currentPosY + anchorY * prevHeight;

            newPosX = constrainPosition(screenX - anchorX * scaledWidth, scaledWidth, containerWidth);
            newPosY = constrainPosition(screenY - anchorY * scaledHeight, scaledHeight, containerHeight);
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
            autoAlignment={{ animationTime: 0 }}
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
                    alt={t('common.previewAlt')}
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
