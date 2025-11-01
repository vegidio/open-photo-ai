import { useImageStore } from '@/stores';

export const PreviewImageSideBySide = () => {
    const originalImage = useImageStore((state) => state.originalImage);
    const enhancedImage = useImageStore((state) => state.enhancedImage ?? state.originalImage);

    return (
        <div className='flex flex-row h-full w-full p-1 gap-1'>
            <div className='flex-1 flex items-center justify-center'>
                {originalImage && (
                    <img alt='Preview' src={originalImage.url} className='w-full h-full object-contain' />
                )}
            </div>

            <div className='flex-1 flex items-center justify-center'>
                {enhancedImage && (
                    <img alt='Preview' src={enhancedImage.url} className='w-full h-full object-contain' />
                )}
            </div>
        </div>
    );
};
