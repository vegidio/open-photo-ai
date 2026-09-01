import { useCallback } from 'react';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { useSyncFaces } from '@/hooks/useSyncFaces.ts';
import { useEnhancementStore } from '@/stores';
import { getEnhancementType } from '@/utils/enhancement.ts';
import { faceRecoveryPrecision } from '@/utils/face.ts';

/** Who asked for the enhancement: a deliberate pick from the menu, or one of Autopilot's suggestions. */
export type EnhancementSource = 'manual' | 'autopilot';

/**
 * Adds enhancements to a file and, when a face-recovery operation is among them, detects the file's faces (against its
 * current crop) up front and stores them in the enhancement store so the UI has them ready.
 *
 * Detection is non-blocking to the add: the enhancement is always applied. Error/empty handling lives in useSyncFaces.
 *
 * `source` separates the two callers in analytics. Autopilot adds every suggestion through this same path, so without
 * it one autopilot run looked like several deliberate picks and swamped the manual numbers - which is exactly the
 * comparison the event is there to support.
 */
export const useAddEnhancements = () => {
    const addEnhancements = useEnhancementStore((s) => s.addEnhancements);
    const syncFaces = useSyncFaces();

    return useCallback(
        async (file: File, operations: Operation[], source: EnhancementSource = 'manual') => {
            addEnhancements(file, operations);

            for (const op of operations) {
                track(AnalyticsEvent.EnhancementAdded, {
                    type: getEnhancementType(op.id) ?? 'unknown',
                    source,
                });
            }

            // The detection pass runs at the face-recovery operation's own precision, so an undefined result is
            // exactly the "nothing here needs faces" case the `hasFaceRecovery` check used to cover.
            const precision = faceRecoveryPrecision(operations.map((op) => op.id));
            if (precision) {
                await syncFaces(file, precision);
            }
        },
        [addEnhancements, syncFaces],
    );
};
