import { useCallback, useEffect, useState } from "react";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useFacesStore } from "@/stores/faces";

/**
 * The working copy of the choice the Select faces dialog edits, and the one way to commit it.
 *
 * **Apply commits and dismissal discards.** Discarding is nothing happening: there is no unwind and
 * no snapshot to restore, because a toggle never touches the store - the property
 * `useCropController` has.
 *
 * **Re-seeded on each open**, so a dismissed edit is gone rather than waiting in a set nobody reset,
 * and **committed only when the set actually differs** from what is already stored, so opening the
 * dialog to read the selection and applying it leaves the run in flight alone.
 *
 * `identity` is the photograph the choice belongs to. A photograph with none has no pixels to serve
 * and nothing to detect in, so there is nothing to choose among; `apply` writes nothing for it.
 */
export const useFaceSelection = (identity: string | undefined, open: boolean) => {
    /*
     * The working copy exists, here and in the reference, because writing the store per box clicked
     * would start an inference run per box.
     *
     * Apply commits and dismissal discards, a deliberate divergence from the reference: its
     * `FaceToggle` commits from `handleClose`, so every way out of the dialog is a commit and there is
     * no way to change one's mind. Screen 13b draws a footer with an Apply carrying the count, and an
     * Apply button means nothing unless dismissal is the other answer. It is also the shape Crop/Rotate
     * has, so the two dialogs a user can open over a photograph answer dismissal the same way.
     */
    const [skipped, setSkipped] = useState<ReadonlySet<string>>(() => new Set());

    /*
     * Read off the store rather than subscribed to, as `useEnhancementRun` reads `setFaces`: what
     * this needs is the writer, and a selector would make the callbacks below depend on a function
     * identity zustand is free to change.
     */
    const setSkippedFaces = useFacesStore.getState().setSkippedFaces;

    /** What the store holds for this photograph right now, which is both what is seeded and what is compared. */
    const committed = useCallback(
        () => (identity && useFacesStore.getState().skipped.get(identity)) || new Set<string>(),
        [identity],
    );

    useEffect(() => {
        if (open) setSkipped(new Set(committed()));
    }, [open, committed]);

    const toggle = useCallback((face: Face) => {
        const key = faceKey(face);

        setSkipped((previous) => {
            const next = new Set(previous);

            if (!next.delete(key)) next.add(key);

            return next;
        });
    }, []);

    const apply = useCallback(() => {
        if (!identity) return;

        // Re-seeding on open and committing only a changed set are both the reference's, and the second
        // is what the `gui-faces` requirement about an unchanged choice asks for.
        const already = committed();
        const changed = already.size !== skipped.size || [...skipped].some((key) => !already.has(key));

        if (changed) setSkippedFaces(identity, skipped);
    }, [identity, committed, skipped, setSkippedFaces]);

    return { skipped, toggle, apply };
};
