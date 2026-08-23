import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import { AnalyticsEvent, track } from '@/analytics';
import { useNotify } from '@/hooks/useNotify.ts';
import i18n from '@/i18n';
import { useCropStore, useEnhancementStore, useSettingsStore } from '@/stores';
import { userFriendlyErrorKey } from '@/utils/errors.ts';
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
            const crop = useCropStore.getState().crops.get(file.Path);

            try {
                const faces = await detectFaces(file, ep, crop);
                setFaces(file, faces);

                // How often face recovery finds anything, and how many faces a typical photo has - which is what
                // decides whether the per-face cost of the recovery models is worth optimising.
                track(AnalyticsEvent.FacesDetected, { count: faces.length });
            } catch (e) {
                console.error('Face detection failed', e);
                setFaces(file, []);
                // The i18n singleton rather than useTranslation's t: this callback is memoised with a dependency
                // list, and adding t would rebuild it on every language change for no benefit.
                enqueueSnackbar(i18n.t(userFriendlyErrorKey(e, 'errors.faceDetectFailed')), { variant: 'error' });
            }
        },
        [setFaces, ep, enqueueSnackbar],
    );
};
