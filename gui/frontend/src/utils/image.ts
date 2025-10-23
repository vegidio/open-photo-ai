import xxhash from 'xxhash-wasm';
import type { DialogFile } from '../../bindings/gui/types';
import { GetImage, ProcessImage } from '../../bindings/gui/services/imageservice.ts';

const binaryCache = new Map<string, string>();
const urlCache = new Map<bigint, string>();
const hasher = await xxhash();

export const getImage = async (file: DialogFile, size: number) => {
    const cacheKey = `${file.Hash}_${size}`;
    let base64 = binaryCache.get(cacheKey);

    if (!base64) {
        base64 = await GetImage(file.Path, size);
        binaryCache.set(cacheKey, base64);
    }

    return createObjectUrl(base64);
};

export const getEnhancedImage = async (file: DialogFile, ...operations: string[]) => {
    const opIds = operations.join('_');
    const cacheKey = `${file.Hash}_${opIds}`;
    let base64 = binaryCache.get(cacheKey);

    if (!base64) {
        base64 = await ProcessImage(file.Path, ...operations);
        binaryCache.set(cacheKey, base64);
    }

    return createObjectUrl(base64);
};

export const clearCache = () => {
    binaryCache.clear();
    urlCache.clear();
};

const createObjectUrl = async (base64: string) => {
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
