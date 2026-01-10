import { ReactCompareSlider } from 'react-compare-slider';
import type { PreviewImageProps } from '@/components/organisms/PreviewImage';
import { ZoomImage } from '@/components/molecules/ZoomImage';

export const PreviewImageSplit = ({ originalImage, enhancedImage, transform }: PreviewImageProps) => {
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
};
