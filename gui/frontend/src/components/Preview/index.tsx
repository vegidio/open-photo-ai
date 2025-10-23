import { useEffect } from 'react';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { PreviewEmpty } from './PreviewEmpty';
import { PreviewImageSideBySide } from './PreviewImageSideBySide.tsx';
import { useControlStore, useFileStore } from '@/stores';
import { getEnhancedImage, getImage } from '@/utils/image.ts';

export const Preview = ({ className = '' }: TailwindProps) => {
    const files = useFileStore((state) => state.files);
    const selectedIndex = useFileStore((state) => state.selectedIndex);
    const setOriginalImage = useFileStore((state) => state.setOriginalImage);
    const setEnhancedImage = useFileStore((state) => state.setEnhancedImage);
    const selectedFile = files.length > 0 ? files[selectedIndex] : undefined;
    const autopilot = useControlStore((state) => state.autopilot);

    // autopilot is intentionally not included in the dependency array because we don't want to re-render the preview if
    // the user switches on/off the autopilot. Only clicking on a different image should trigger a re-render.
    // biome-ignore lint/correctness/useExhaustiveDependencies: N/A autopilot
    useEffect(() => {
        async function loadPreview() {
            if (selectedFile) {
                const originalImage = await getImage(selectedFile, 0);

                // We set both images to the original image for now, later we will determine if we need to display the
                // enhanced image or not based on the autopilot state.
                setOriginalImage(originalImage);
                setEnhancedImage(originalImage);

                if (autopilot) {
                    const enhancedImage = await getEnhancedImage(selectedFile, 'upscale_4_high');
                    setEnhancedImage(enhancedImage);
                }
            } else {
                setOriginalImage(undefined);
                setEnhancedImage(undefined);
            }
        }

        loadPreview();
    }, [setEnhancedImage, setOriginalImage, selectedFile]);

    return (
        <div
            id='preview'
            className={`flex items-center justify-center bg-[#171717] [background-image:radial-gradient(#383838_1px,transparent_1px)] [background-size:3rem_3rem] ${className}`}
        >
            {files.length === 0 ? <PreviewEmpty /> : <PreviewImageSideBySide />}
        </div>
    );
};
