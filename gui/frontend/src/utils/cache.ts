// In-memory cache for the image binaries
export const binaryCache = new Map<string, string>();

// In-memory cache for the image URLs
export const urlCache = new Map<bigint, string>();

// Clear the in-memory cache
export const clearCache = () => {
    binaryCache.clear();
    urlCache.clear();
};
