import { useFileStore } from '@/stores';

export const PreviewImageSideBySide = () => {
    const originalImage = useFileStore((state) => state.originalImage);
    const enhancedImage = useFileStore((state) => state.enhancedImage);
    const secondImage = enhancedImage ?? originalImage;

    return (
        <div className='flex flex-row h-full w-full p-1 gap-1'>
            <div className='flex-1 flex items-center justify-center'>
                {originalImage && <img alt='Preview' src={originalImage} className='w-full h-full object-contain' />}
            </div>

            <div className='flex-1 flex items-center justify-center'>
                {secondImage && <img alt='Preview' src={secondImage} className='w-full h-full object-contain' />}
            </div>
        </div>
    );
};
