import { useEffect, useState } from 'react';
import type { TailwindProps } from '@/utils';

type ImageBase64Props = TailwindProps & {
    base64: string;
};

export const ImageBase64 = ({ base64, className = '' }: ImageBase64Props) => {
    const [imageUrl, setImageUrl] = useState<string>();

    useEffect(() => {
        let objectUrl: string | undefined;

        async function loadImage() {
            try {
                const response = await fetch(`data:application/octet-stream;base64,${base64}`);
                const blob = await response.blob();
                objectUrl = URL.createObjectURL(blob);
                setImageUrl(objectUrl);
            } catch (e) {
                setImageUrl(undefined);
                console.error(e);
            }
        }

        loadImage();

        return () => {
            if (objectUrl) URL.revokeObjectURL(objectUrl);
        };
    }, [base64]);

    return <>{imageUrl && <img alt='Preview' src={imageUrl} className={`${className}`} />}</>;
};
