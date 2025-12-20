import { useEffect } from 'react';
import { CancelError, type CancellablePromise } from '@wailsio/runtime';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';
import { PreviewProgress } from './PreviewProgress.tsx';
import { useDrawerStore, useEnhancementStore, useFileStore, useImageStore } from '@/stores';
import { getEnhancedImage, getImage, type ImageData } from '@/utils/image.ts';

const EMPTY_OPERATIONS: Operation[] = [];

export const Preview = ({ className = '' }: TailwindProps) => {
    // FileListStore
    const filesLength = useFileStore((state) => state.files.length);
    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const setOpen = useDrawerStore((state) => state.setOpen);

    // ImageStore
    const running = useImageStore((state) => state.running);
    const setIsRunning = useImageStore((state) => state.setIsRunning);
    const setOriginalImage = useImageStore((state) => state.setOriginalImage);
    const setEnhancedImage = useImageStore((state) => state.setEnhancedImage);

    // EnhancementStore
    const operations = useEnhancementStore((state) =>
        currentFile ? (state.enhancements.get(currentFile) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );

    useEffect(() => {
        let p: CancellablePromise<ImageData>;

        async function loadPreview() {
            if (currentFile) {
                const originalImage = await getImage(currentFile, 0);
                setOriginalImage(originalImage);

                if (operations.length > 0) {
                    setIsRunning(true);

                    const opIds = operations.map((op) => op.id);
                    p = getEnhancedImage(currentFile, ...opIds);

                    try {
                        const enhancedImage = await p;
                        setEnhancedImage(enhancedImage);
                    } catch (e) {
                        if (!(e instanceof CancelError)) console.error('Error loading enhanced image', e);
                    } finally {
                        setIsRunning(false);
                    }
                } else {
                    // When there are no operations, use the original image
                    setEnhancedImage(originalImage);
                }
            } else {
                setOriginalImage(undefined);
                setEnhancedImage(undefined);
            }
        }

        loadPreview();

        return () => {
            // Cancel any pending request to preview the enhanced image
            p?.cancel();
        };
    }, [operations, currentFile, setEnhancedImage, setIsRunning, setOriginalImage]);

    // useEffect(() => {
    //     if (filesLength > 0) setOpen(true);
    // }, [filesLength, setOpen]);

    return (
        <div
            id='preview'
            className={`flex items-center justify-center bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {running && <PreviewProgress />}
            {filesLength === 0 ? <PreviewEmpty /> : <PreviewImageSideBySide />}
        </div>
    );
};
