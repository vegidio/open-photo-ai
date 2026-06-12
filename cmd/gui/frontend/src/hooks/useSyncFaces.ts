import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import { useNotify } from '@/hooks/useNotify.ts';
import { useCropStore, useEnhancementStore, useSettingsStore } from '@/stores';
import { userFriendlyErrorMessage } from '@/utils/errors.ts';
import { detectFaces } from '@/utils/face.ts';

/**
 * Returns a callback that (re)detects a file's faces against its current crop and stores them in the enhancement store
 * so the UI (face count, face overlay) stays in sync.
 *
 * Detection runs on the cropped image, so a crop change re-numbers the faces; `setFaces` resets any prior face
 * de-selection, which is the intended behavior. On failure an empty array is stored and an error snackbar is shown;
 * when no faces are found an empty array is stored silently.
 */
export const useSyncFaces = () => {
    const { enqueueSnackbar } = useNotify();
    const setFaces = useEnhancementStore((s) => s.setFaces);
    const ep = useSettingsStore((s) => s.executionProvider);

    return useCallback(
        async (file: File) => {
            const crop = useCropStore.getState().crops.get(file);

            try {
                const faces = await detectFaces(file, ep, crop);
                setFaces(file, faces);
            } catch (e) {
                setFaces(file, []);
                const msg = userFriendlyErrorMessage(e, 'Something went wrong. Failed to detect faces.');
                enqueueSnackbar(msg, { variant: 'error' });
            }
        },
        [setFaces, ep, enqueueSnackbar],
    );
};
