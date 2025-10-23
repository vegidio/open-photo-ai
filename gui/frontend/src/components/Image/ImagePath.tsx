import { useEffect, useState } from 'react';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import type { DialogFile } from '../../../bindings/gui/types';
import { GetImage } from '../../../bindings/gui/services/imageservice.ts';
import { ImageBase64 } from './ImageBase64';
import { pathCache } from '@/utils/cache.ts';

type ImagePathProps = TailwindProps & {
    file: DialogFile;
    size?: number;
};

export const ImagePath = ({ file, size = 0, className = '' }: ImagePathProps) => {
    const cacheKey = `${file.Hash}_${size}`;
    const [base64, setBase64] = useState<string>();

    useEffect(() => {
        // Check cache first
        if (pathCache.has(cacheKey)) {
            setBase64(pathCache.get(cacheKey));
            return;
        }

        async function loadFile() {
            try {
                const b64 = await GetImage(file.Path, size);

                // Store in cache
                pathCache.set(cacheKey, b64);
                setBase64(b64);
            } catch (e) {
                setBase64(undefined);
                console.error(e);
            }
        }

        loadFile();
    }, [cacheKey, file, size]);

    return <>{base64 && <ImageBase64 base64={base64} className={className} />}</>;
};
