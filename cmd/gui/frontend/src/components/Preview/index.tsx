import { useCallback, useEffect, useState } from 'react';
import { LinearProgress, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';
import { useDrawerStore, useEnhancementStore, useFileStore, useImageStore } from '@/stores';
import { getEnhancedImage, getImage } from '@/utils/image.ts';

const EMPTY_OPERATIONS: Operation[] = [];

export const Preview = ({ className = '' }: TailwindProps) => {
    // FileListStore
    const filesLength = useFileStore((state) => state.files.length);
    const selectedFile = useFileStore((state) =>
        state.files.length > 0 ? state.files[state.selectedIndex] : undefined,
    );
    const setOpen = useDrawerStore((state) => state.setOpen);

    // ImageStore
    const running = useImageStore((state) => state.running);
    const setIsRunning = useImageStore((state) => state.setIsRunning);
    const setOriginalImage = useImageStore((state) => state.setOriginalImage);
    const setEnhancedImage = useImageStore((state) => state.setEnhancedImage);

    // EnhancementStore
    const operations = useEnhancementStore((state) =>
        selectedFile ? (state.enhancements.get(selectedFile) ?? EMPTY_OPERATIONS) : EMPTY_OPERATIONS,
    );

    useEffect(() => {
        async function loadPreview() {
            if (selectedFile) {
                const originalImage = await getImage(selectedFile, 0);

                // We set both images to the original image for now, later we will determine if we need to display the
                // enhanced image or not based on the autopilot state.
                setOriginalImage(originalImage);
                setEnhancedImage(originalImage);

                if (operations.length > 0) {
                    setIsRunning(true);

                    const opIds = operations.map((op) => op.id);
                    const enhancedImage = await getEnhancedImage(selectedFile, ...opIds);
                    setEnhancedImage(enhancedImage);

                    setIsRunning(false);
                }
            } else {
                setOriginalImage(undefined);
                setEnhancedImage(undefined);
            }
        }

        loadPreview();
    }, [selectedFile, setEnhancedImage, setIsRunning, setOriginalImage, operations]);

    // useEffect(() => {
    //     if (filesLength > 0) setOpen(true);
    // }, [filesLength, setOpen]);

    return (
        <div
            id='preview'
            className={`flex items-center justify-center bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {running && <ProgressUpdate />}
            {filesLength === 0 ? <PreviewEmpty /> : <PreviewImageSideBySide />}
        </div>
    );
};

const ProgressUpdate = () => {
    const [progress, setProgress] = useState({ name: 'Enhancing', value: 0 });

    const getOperationName = useCallback((id: string) => {
        switch (id) {
            case 'upscale':
                return 'Upscale';
            case 'face-recovery':
                return 'Face Recovery';
            default:
                return 'Enhancing';
        }
    }, []);

    useEffect(() => {
        Events.On('app:progress', (event) => {
            const [id, value] = event.data as [string, number];
            setProgress({ name: getOperationName(id), value: value * 100 });
        });

        return () => Events.Off('app:progress');
    }, [getOperationName]);

    return (
        <div className='absolute flex top-4 right-4 w-32 h-7 items-center justify-center shadow-xl'>
            <LinearProgress variant='determinate' value={progress.value} className='size-full rounded-[5px]' />
            <Typography variant='subtitle2' className='absolute text-gray-700'>
                {progress.name}
            </Typography>
        </div>
    );
};
