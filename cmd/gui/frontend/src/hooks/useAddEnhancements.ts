import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { useSyncFaces } from '@/hooks/useSyncFaces.ts';
import { useEnhancementStore } from '@/stores';
import { hasFaceRecovery } from '@/utils/face.ts';

/**
 * Adds enhancements to a file and, when a face-recovery operation is among them, detects the file's faces (against its
 * current crop) up front and stores them in the enhancement store so the UI has them ready.
 *
 * Detection is non-blocking to the add: the enhancement is always applied. Error/empty handling lives in useSyncFaces.
 */
export const useAddEnhancements = () => {
    const addEnhancements = useEnhancementStore((s) => s.addEnhancements);
    const syncFaces = useSyncFaces();

    return useCallback(
        async (file: File, operations: Operation[]) => {
            addEnhancements(file, operations);

            if (hasFaceRecovery(operations.map((op) => op.id))) {
                await syncFaces(file);
            }
        },
        [addEnhancements, syncFaces],
    );
};
