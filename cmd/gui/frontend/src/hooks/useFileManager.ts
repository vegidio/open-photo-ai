import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import { useEnhancementStore, useFileStore, useImageStore } from '@/stores';

// useFileManager is the single place callers should go to remove files or clear the workspace.
// It keeps the file list, enhancements, and per-image transforms in sync without coupling the
// stores to each other at module scope.
export const useFileManager = () => {
    const removeFileFromList = useFileStore((state) => state.removeFile);
    const removeSelectedFile = useFileStore((state) => state.removeSelectedFile);
    const clearFileList = useFileStore((state) => state.clear);
    const removeEnhancementsKey = useEnhancementStore((state) => state.removeKey);
    const clearEnhancements = useEnhancementStore((state) => state.clear);
    const removeImageTransform = useImageStore((state) => state.removeImageTransform);
    const clearImageState = useImageStore((state) => state.clear);

    const removeFile = useCallback(
        (file: File) => {
            removeFileFromList(file);
            removeSelectedFile(file.Path);
            removeEnhancementsKey(file);
            removeImageTransform(file.Hash);
        },
        [removeFileFromList, removeSelectedFile, removeEnhancementsKey, removeImageTransform],
    );

    const clearAll = useCallback(() => {
        clearFileList();
        clearEnhancements();
        clearImageState();
    }, [clearFileList, clearEnhancements, clearImageState]);

    return { removeFile, clearAll };
};
