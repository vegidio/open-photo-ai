import type { PreviewImageProps } from '@/components/organisms/PreviewImage';
import { ZoomImage } from '@/components/molecules/ZoomImage';

export const PreviewImageFull = ({ enhancedImage, transform }: PreviewImageProps) => {
    return (
        <div id='preview_body' className='flex flex-row size-full p-0.5'>
            {enhancedImage && <ZoomImage image={enhancedImage} imageTransform={transform} />}
        </div>
    );
};
