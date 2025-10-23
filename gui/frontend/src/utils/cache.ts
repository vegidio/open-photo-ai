// In-memory cache for the file paths
export const pathCache = new Map<string, string>();

// In-memory cache for the file binary data
export const binaryCache = new Map<bigint, string>();

// Clear the in-memory cache
export const clearCache = () => {
    pathCache.clear();
    binaryCache.clear();
};
