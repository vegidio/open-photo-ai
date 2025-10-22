import { useCallback, useEffect, useState } from 'react';
import { ProcessImage } from '../../../bindings/gui/services/imageservice.ts';
import { ImageBase64, ImagePath } from '@/components/Image';
import { useControlStore, useFileStore } from '@/stores';

export const PreviewImageSideBySide = () => {
    const filePaths = useFileStore((state) => state.filePaths);
    const selectedIndex = useFileStore((state) => state.selectedIndex);
    const originalImage = useFileStore((state) => state.originalImage);
    const enhancedImage = useFileStore((state) => state.enhancedImage);
    const setOriginalImage = useFileStore((state) => state.setOriginalImage);
    const setEnhancedImage = useFileStore((state) => state.setEnhancedImage);
    const autopilot = useControlStore((state) => state.autopilot);

    const [base64Image, setBase64Image] = useState<string>();

    const processImage = useCallback(async () => {
        if (filePaths.length > 0) {
            const selectedPath = filePaths[selectedIndex];
            const base64 = await ProcessImage(selectedPath);
            setBase64Image(base64);
        } else {
            setBase64Image(undefined);
        }
    }, [filePaths, selectedIndex]);

    // biome-ignore lint/correctness/useExhaustiveDependencies: Autopilot should run only once
    useEffect(() => {
        if (autopilot) processImage();
    }, [processImage]);

    return (
        <div className='flex flex-row h-full w-full p-1 gap-1'>
            <div className='flex-1 flex items-center justify-center'>
                {filePaths.length > 0 && (
                    <ImagePath path={filePaths[selectedIndex]} className='w-full h-full object-contain' />
                )}
            </div>

            <div className='flex-1 flex items-center justify-center'>
                {(() => {
                    if (filePaths.length > 0) {
                        return autopilot && base64Image ? (
                            <ImageBase64 base64={base64Image} className='w-full h-full object-contain' />
                        ) : (
                            <ImagePath path={filePaths[selectedIndex]} className='w-full h-full object-contain' />
                        );
                    } else {
                        return null;
                    }
                })()}
            </div>
        </div>
    );
};
