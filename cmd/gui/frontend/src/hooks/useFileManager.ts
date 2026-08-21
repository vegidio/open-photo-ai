import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import { AnalyticsEvent, track } from '@/analytics';
import { DialogService } from '@/bindings/gui/services';
import { useCropStore, useEnhancementStore, useFileStore, useImageStore } from '@/stores';
import { clearFacesCache } from '@/utils/face.ts';
import { clearCache as clearImageCache } from '@/utils/image.ts';

// useFileManager is the single place callers should go to open, remove or clear files.
// It keeps the file list, enhancements, crops, and per-image transforms in sync without coupling the
// stores to each other at module scope.
export const useFileManager = () => {
    const { t } = useTranslation();
    const addFilesToList = useFileStore((state) => state.addFiles);
    const removeFileFromList = useFileStore((state) => state.removeFile);
    const removeSelectedFile = useFileStore((state) => state.removeSelectedFile);
    const clearFileList = useFileStore((state) => state.clear);
    const removeEnhancementsKey = useEnhancementStore((state) => state.removeKey);
    const clearEnhancements = useEnhancementStore((state) => state.clear);
    const removeCropKey = useCropStore((state) => state.removeKey);
    const clearCrops = useCropStore((state) => state.clear);
    const removeImageTransform = useImageStore((state) => state.removeImageTransform);
    const clearImageState = useImageStore((state) => state.clear);

    const removeFile = useCallback(
        (file: File) => {
            removeFileFromList(file);
            removeSelectedFile(file.Path);
            removeEnhancementsKey(file);
            removeCropKey(file);
            removeImageTransform(file.Hash);
            track(AnalyticsEvent.FileRemoved);
        },
        [removeFileFromList, removeSelectedFile, removeEnhancementsKey, removeCropKey, removeImageTransform],
    );

    const clearAll = useCallback(() => {
        clearFileList();
        clearEnhancements();
        clearCrops();
        clearImageState();

        // The stores hold references to files; the caches hold the decoded pixels behind them. Clearing only the
        // stores leaves the object URLs and detection results resident for files the user can no longer reach.
        clearImageCache();
        clearFacesCache();

        track(AnalyticsEvent.FilesCleared);
    }, [clearFileList, clearEnhancements, clearCrops, clearImageState]);

    // The native dialog's copy is passed in from here because the backend has no i18n catalog of its own; doing it
    // in one place is what keeps the two browse buttons from drifting apart on their wording or their analytics.
    const openFiles = useCallback(
        async (source: string) => {
            try {
                const files = await DialogService.OpenFileDialog(
                    t('dialogs.native.selectImage'),
                    t('dialogs.native.imagesFilter'),
                );

                addFilesToList(files);
                if (files.length > 0) track(AnalyticsEvent.FilesAdded, { count: files.length, source });
            } catch (e) {
                console.error(e);
            }
        },
        [addFilesToList, t],
    );

    return { openFiles, removeFile, clearAll };
};
