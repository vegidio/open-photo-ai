import { ZoomImage } from '@/components/molecules/ZoomImage';
import { useFileStore, useImageStore } from '@/stores';

const INITIAL_TRANSFORM = { scale: 1, positionX: 0, positionY: 0 };

export const PreviewImageSideBySide = () => {
    const originalImage = useImageStore((state) => state.originalImage);
    const enhancedImage = useImageStore((state) => state.enhancedImage);

    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const imageTransform =
        useImageStore((state) => state.imageTransform.get(currentFile?.Hash ?? '')) ?? INITIAL_TRANSFORM;

    return (
        <div id='preview_body' className='flex flex-row h-full w-full p-1 gap-1'>
            {originalImage && <ZoomImage image={originalImage} imageTransform={imageTransform} />}
            {enhancedImage && <ZoomImage image={enhancedImage} imageTransform={imageTransform} />}
        </div>
    );
};
