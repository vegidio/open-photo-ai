import xxhash from 'xxhash-wasm';
import type { DialogFile } from '../../bindings/gui/types';
import { GetImage } from '../../bindings/gui/services/imageservice.ts';
import { binaryCache, urlCache } from '@/utils/cache.ts';

const hasher = await xxhash();

export const getFileImage = async (file: DialogFile, size: number) => {
    const cacheKey = `${file.Hash}_${size}`;
    let base64 = binaryCache.get(cacheKey);

    if (!base64) {
        base64 = await GetImage(file.Path, size);
        binaryCache.set(cacheKey, base64);
    }

    return getBase64Image(base64);
};

export const getBase64Image = async (base64: string) => {
    const hash = hasher.h64(base64);
    let url = urlCache.get(hash);

    if (!url) {
        const response = await fetch(`data:application/octet-stream;base64,${base64}`);
        const blob = await response.blob();
        url = URL.createObjectURL(blob);
        urlCache.set(hash, url);
    }

    return url;
};
