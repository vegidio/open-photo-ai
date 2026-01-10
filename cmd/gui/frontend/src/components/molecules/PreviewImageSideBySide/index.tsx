import type { PreviewImageProps } from '@/components/organisms/PreviewImage';
import { ZoomImage } from '@/components/molecules/ZoomImage';

export const PreviewImageSideBySide = ({ originalImage, enhancedImage, transform }: PreviewImageProps) => {
    return (
        <div id='preview_body' className='flex flex-row size-full p-0.5 gap-0.5'>
            {originalImage && <ZoomImage image={originalImage} imageTransform={transform} />}
            {enhancedImage && <ZoomImage image={enhancedImage} imageTransform={transform} />}
        </div>
    );
};
