import { ReactCompareSlider } from 'react-compare-slider';
import { ZoomImage } from '@/components/organisms/ZoomImage';
import { useAppStore, useFileStore, useImageStore } from '@/stores';

const INITIAL_TRANSFORM = { scale: 1, positionX: 0, positionY: 0 };

export const PreviewImage = () => {
    const previewMode = useAppStore((state) => state.previewMode);

    const originalImage = useImageStore((state) => state.originalImage);
    const enhancedImage = useImageStore((state) => state.enhancedImage);

    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const transform = useImageStore((state) => state.imageTransform.get(currentFile?.Hash ?? '')) ?? INITIAL_TRANSFORM;

    switch (previewMode) {
        case 'full':
            return (
                <div id='preview_body' className='flex flex-row size-full p-0.5'>
                    {enhancedImage && <ZoomImage image={enhancedImage} imageTransform={transform} />}
                </div>
            );

        case 'side':
            return (
                <div id='preview_body' className='flex flex-row size-full p-0.5 gap-0.5'>
                    {originalImage && <ZoomImage image={originalImage} imageTransform={transform} />}
                    {enhancedImage && <ZoomImage image={enhancedImage} imageTransform={transform} />}
                </div>
            );

        case 'split':
            return (
                <div className='size-full p-0.5'>
                    <ReactCompareSlider
                        onlyHandleDraggable={true}
                        className='size-full'
                        itemOne={originalImage && <ZoomImage image={originalImage} imageTransform={transform} />}
                        itemTwo={enhancedImage && <ZoomImage image={enhancedImage} imageTransform={transform} />}
                    />
                </div>
            );
    }
};
