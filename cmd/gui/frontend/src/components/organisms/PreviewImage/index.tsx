import type { ImageData } from '@/utils/image.ts';
import { PreviewImageFull } from '@/components/molecules/PreviewImageFull';
import { PreviewImageSideBySide } from '@/components/molecules/PreviewImageSideBySide';
import { type ImageTransform, useAppStore, useFileStore, useImageStore } from '@/stores';

const INITIAL_TRANSFORM = { scale: 1, positionX: 0, positionY: 0 };

export type PreviewImageProps = {
    originalImage?: ImageData;
    enhancedImage?: ImageData;
    transform: ImageTransform;
};

export const PreviewImage = () => {
    const previewMode = useAppStore((state) => state.previewMode);

    const originalImage = useImageStore((state) => state.originalImage);
    const enhancedImage = useImageStore((state) => state.enhancedImage);

    const currentFile = useFileStore((state) => state.files.at(state.currentIndex));
    const transform = useImageStore((state) => state.imageTransform.get(currentFile?.Hash ?? '')) ?? INITIAL_TRANSFORM;

    switch (previewMode) {
        case 'full':
            return <PreviewImageFull enhancedImage={enhancedImage} transform={transform} />;

        case 'side':
        case 'split':
            return (
                <PreviewImageSideBySide
                    originalImage={originalImage}
                    enhancedImage={enhancedImage}
                    transform={transform}
                />
            );
    }
};
