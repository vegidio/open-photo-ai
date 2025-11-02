import { ZoomImage } from '@/components/ZoomImage';
import { useImageStore } from '@/stores';

export const PreviewImageSideBySide = () => {
    const originalImage = useImageStore((state) => state.originalImage);
    const enhancedImage = useImageStore((state) => state.enhancedImage ?? state.originalImage);

    return (
        <div id='preview_body' className='flex flex-row h-full w-full p-1 gap-1'>
            {originalImage && <ZoomImage key='original' image={originalImage} />}
            {enhancedImage && <ZoomImage key='enhanced' image={enhancedImage} />}
        </div>
    );
};
