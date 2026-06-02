import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { useNotify } from '@/hooks/useNotify.ts';
import { useEnhancementStore, useSettingsStore } from '@/stores';
import { userFriendlyErrorMessage } from '@/utils/errors.ts';
import { detectFaces, hasFaceRecovery } from '@/utils/face.ts';

/**
 * Adds enhancements to a file and, when a face-recovery operation is among them, detects the file's faces up front and
 * stores them in the enhancement store so the UI has them ready.
 *
 * Detection is non-blocking to the add: the enhancement is always applied. On failure an empty array is stored and an
 * error snackbar is shown; when no faces are found an empty array is stored silently.
 */
export const useAddEnhancements = () => {
    const { enqueueSnackbar } = useNotify();
    const addEnhancements = useEnhancementStore((s) => s.addEnhancements);
    const setFaces = useEnhancementStore((s) => s.setFaces);
    const ep = useSettingsStore((s) => s.executionProvider);

    return useCallback(
        async (file: File, operations: Operation[]) => {
            addEnhancements(file, operations);

            if (hasFaceRecovery(operations.map((op) => op.id))) {
                try {
                    const faces = await detectFaces(file, ep); // cached by file hash
                    setFaces(file, faces);
                } catch (e) {
                    setFaces(file, []);
                    const msg = userFriendlyErrorMessage(e, 'Something went wrong. Failed to detect faces.');
                    enqueueSnackbar(msg, { variant: 'error' });
                }
            }
        },
        [addEnhancements, setFaces, ep, enqueueSnackbar],
    );
};
