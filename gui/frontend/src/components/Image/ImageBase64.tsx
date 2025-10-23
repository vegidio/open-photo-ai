import { useEffect, useState } from 'react';
import xxhash from 'xxhash-wasm';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { binaryCache } from '@/utils/cache.ts';

const hasher = await xxhash();

type ImageBase64Props = TailwindProps & {
    base64: string;
};

export const ImageBase64 = ({ base64, className = '' }: ImageBase64Props) => {
    const [imageUrl, setImageUrl] = useState<string>();

    useEffect(() => {
        const hash = hasher.h64(base64);

        // Check cache first
        if (binaryCache.has(hash)) {
            setImageUrl(binaryCache.get(hash));
            return;
        }

        let objectUrl: string | undefined;

        async function loadImage() {
            try {
                const response = await fetch(`data:application/octet-stream;base64,${base64}`);
                const blob = await response.blob();
                objectUrl = URL.createObjectURL(blob);

                // Store in cache
                binaryCache.set(hash, objectUrl);
                setImageUrl(objectUrl);
            } catch (e) {
                setImageUrl(undefined);
                console.error(e);
            }
        }

        loadImage();
    }, [base64]);

    return <>{imageUrl && <img alt='Preview' src={imageUrl} className={`${className}`} />}</>;
};
