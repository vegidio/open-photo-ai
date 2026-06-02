import { useCallback, useEffect, useState } from 'react';
import type { File } from '@/bindings/gui/types';
import { useEnhancementStore } from '@/stores';

/**
 * Manages a local working copy of the disabled-face selection for the "Select faces" modal.
 *
 * Toggling only mutates the local set (so the boxes recolor instantly without touching the store or re-running
 * inference); `commit` writes the final set to the store, and should be called when the modal closes. This way the user
 * can toggle several faces, and only a single inference runs once the modal is closed.
 */
export const useFaceSelection = (file: File, open: boolean) => {
    const setDisabledFaces = useEnhancementStore((s) => s.setDisabledFaces);
    const [disabled, setDisabled] = useState<Set<number>>(() => new Set());

    // Re-seed the working copy from the committed selection each time the modal opens.
    useEffect(() => {
        if (open) setDisabled(new Set(useEnhancementStore.getState().disabledFaces.get(file) ?? []));
    }, [open, file]);

    const toggle = useCallback((index: number) => {
        setDisabled((prev) => {
            const next = new Set(prev);
            if (next.has(index)) {
                next.delete(index);
            } else {
                next.add(index);
            }
            return next;
        });
    }, []);

    // Commit to the store (call on close). Only writes when the set actually changed, so an open-and-close with no
    // edits doesn't re-trigger the preview.
    const commit = useCallback(() => {
        const committed = useEnhancementStore.getState().disabledFaces.get(file) ?? new Set<number>();
        const changed = disabled.size !== committed.size || [...disabled].some((x) => !committed.has(x));
        if (changed) setDisabledFaces(file, disabled);
    }, [disabled, file, setDisabledFaces]);

    return { disabled, toggle, commit };
};
