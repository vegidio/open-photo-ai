import { useCallback, useEffect, useState } from 'react';
import { ProcessImage } from '../../../bindings/gui/services/imageservice.ts';
import { ImageBase64, ImagePath } from '@/components/Image';
import { useControlStore, useFileStore } from '@/stores';

export const PreviewImageSideBySide = () => {
    const files = useFileStore((state) => state.files);
    const selectedIndex = useFileStore((state) => state.selectedIndex);
    const autopilot = useControlStore((state) => state.autopilot);

    const [base64Image, setBase64Image] = useState<string>();

    const processImage = useCallback(async () => {
        if (files.length > 0) {
            const base64 = await ProcessImage(files[selectedIndex].Path);
            setBase64Image(base64);
        } else {
            setBase64Image(undefined);
        }
    }, [files, selectedIndex]);

    // biome-ignore lint/correctness/useExhaustiveDependencies: Autopilot should run only once
    useEffect(() => {
        // if (autopilot) processImage();
    }, [processImage]);

    return (
        <div className='flex flex-row h-full w-full p-1 gap-1'>
            <div className='flex-1 flex items-center justify-center'>
                {files.length > 0 && <ImagePath file={files[selectedIndex]} className='w-full h-full object-contain' />}
            </div>

            <div className='flex-1 flex items-center justify-center'>
                {(() => {
                    if (files.length > 0) {
                        return autopilot && base64Image ? (
                            <ImageBase64 base64={base64Image} className='w-full h-full object-contain' />
                        ) : (
                            <ImagePath file={files[selectedIndex]} className='w-full h-full object-contain' />
                        );
                    } else {
                        return null;
                    }
                })()}
            </div>
        </div>
    );
};
