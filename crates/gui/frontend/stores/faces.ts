import { create } from "zustand";
import { type CropInfo, sameCrop } from "@/ipc/crop";
import type { FaceChoice } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import { registerFileOwner, useFileStore } from "@/stores/files";

/** The faces found in one photograph, beside the framing they were found at. */
type Detected = {
    /** How the photograph was framed when they were found, or absent where it was looked at whole. */
    crop?: CropInfo;
    /** The faces, in the order the detector found them. Empty for a photograph with nobody in it. */
    faces: Face[];
};

type FacesStore = {
    // Keyed by identity for the reasons `transforms` in `stores/transform.ts` gives: a file whose bytes could not
    // be read has no faces to find in the first place.
    //
    // **The framing travels in the value rather than in the key**, which is `useEnhancementRun`'s own arrangement
    // for the enhanced result and is there for the same reason: an answer found at a different framing simply does
    // not match, so there is no window in which one framing's faces are reported for another's. Keying by
    // `identity + crop` would instead be a cache of every framing the user has passed through, which is a second
    // unbounded cache in the webview - and an unnecessary one, because `opai`'s own run store already serves a
    // framing returned to, is bounded, and outlives the process.
    /**
     * The faces found in each photograph, keyed by identity. Two copies of one photograph in two folders are one
     * thing to look at and share one answer.
     */
    faces: Map<string, Detected>;

    // **A choice names faces by key rather than by their place among the faces found**, which is what makes a
    // framing change need no rule of its own: a flip, a turn or a cut moves every face, so nothing in the new
    // framing matches anything chosen in the old one, and every face in it follows the default. A framing returned
    // to is answered by the faces found there the first time - key for key, since `opai`'s own run store serves it -
    // so the choice made there is back in force. The reference keeps indices instead and therefore carries a choice
    // across a cut *by position*, which promotes whichever face is now second into the slot the user skipped.
    //
    // **Only the user's exceptions to the default**, never the default itself: the default is each face's own
    // `restorable`, which Rust decides and publishes, so a detection - however often it runs - never writes here.
    // That is what lets this be a dependency of the run without a detection landing re-running it, and what keeps
    // a large face the user turned on, turned on after a crop and its undo: the exception is still there, and the
    // default was never recorded to be applied again.
    //
    // **Not keyed by framing**, which follows from the by-key rule: an exception from a framing nobody is at
    // matches nothing and costs one string. It grows only by faces a person clicked, so it is bounded by the
    // photograph.
    //
    // **Here rather than beside the enhancement stack**, because the choice is about the pixels and not about the
    // operation: it names faces, so it belongs where the faces are, and removing a face recovery and adding another
    // leaves it alone.
    /**
     * The choice made among each photograph's faces, keyed by identity: the faces the user turned off although a
     * recovery would restore them, and the ones turned on although it would not.
     *
     * Absent for a photograph nobody has chosen faces in, which is the same answer as a choice with no exceptions
     * and is the one that costs nothing to hold.
     */
    choices: Map<string, FaceChoice>;

    setFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) => void;
    recordFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) => void;
    setFaceChoice: (identity: string, choice: FaceChoice) => void;
    forgetFaces: (identity: string) => void;
    forgetAllFaces: () => void;
};

// **The faces' writers are the run path and the picker**, and both write the same value through the same
// `recordFaces`. `hooks/useEnhancementRun.ts` records what a chain carrying a face recovery found - the run finds them
// itself - and the face-recovery row asks `detect_faces` when its options are open over a photograph whose faces are
// not known yet, so the picker has something to offer before the chain has finished. Nothing else asks for a
// detection, which is what makes "faces are found only for a photograph whose enhancements need them" structural.
//
// Every write replaces a map rather than mutating it: see `setTransform` in `stores/transform.ts`.
//
// Not persisted because faces belong to a photograph that is open, and coordinates that outlived a restart would
// describe pixels nobody has loaded.
/**
 * The faces in each open photograph, as they were found at the framing in force, and the choice made among them.
 *
 * **Not persisted**, and released when the photograph is closed.
 */
export const useFacesStore = create<FacesStore>()((set, get) => ({
    faces: new Map<string, Detected>(),
    choices: new Map<string, FaceChoice>(),

    /**
     * Records the faces found in one photograph at one framing.
     *
     * **Records them and nothing else.** The default among them is each face's `restorable`, so there is nothing
     * to decide here, and the choice is written only by a person.
     *
     * An empty array is a legitimate value and is written like any other: it is what a photograph with nobody in
     * it answers, and what a failed detection is recorded as - so that "are this photograph's faces known?" has an
     * answer.
     */
    setFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) =>
        set((state) => ({ faces: new Map(state.faces).set(identity, { ...(crop && { crop }), faces }) })),

    /**
     * Records the faces a detection answered with, as `setFaces` does - **but only while the photograph is still
     * open**, as every late answer is: a photograph that was closed has had every owner told to forget it.
     *
     * **Writes nothing where the same faces are already recorded at the same framing**, compared by key, so a
     * detection answered again - by the picker and then by the chain, say - does not re-render every subscriber
     * over an equal value.
     */
    recordFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) => {
        if (!useFileStore.getState().files.some((open) => open.identity === identity)) return;

        const recorded = get().faces.get(identity);
        const unchanged =
            recorded &&
            sameCrop(recorded.crop, crop) &&
            recorded.faces.length === faces.length &&
            recorded.faces.every((face, index) => face.key === faces[index]?.key);

        if (!unchanged) get().setFaces(identity, crop, faces);
    },

    /**
     * Records the choice made among one photograph's faces.
     *
     * **The whole choice is replaced on every write**, as the whole map is. A new object as well as a new map, for
     * the rule `setTransform` in `stores/transform.ts` states: here a subscriber comparing references is what
     * `useEnhancementRun` depends on to re-run the chain exactly once per applied choice and at no other time.
     */
    setFaceChoice: (identity: string, choice: FaceChoice) =>
        set((state) => ({
            choices: new Map(state.choices).set(identity, {
                skipped: [...choice.skipped],
                restored: [...choice.restored],
            }),
        })),

    /**
     * Forgets the faces found in one photograph and the choice made among them, which is what closing it means. A
     * photograph opened again is asked about afresh.
     *
     * A no-op for a file with no identity and for one nothing was ever detected in: neither has an entry.
     */
    forgetFaces: (identity: string) =>
        set((state) => {
            // Both go together: a choice left behind would decide faces nobody has chosen about this time round.
            // Replaced even when nothing was deleted, as `forgetTransform` does and for its reason.
            const faces = new Map(state.faces);
            faces.delete(identity);

            const choices = new Map(state.choices);
            choices.delete(identity);

            return { faces, choices };
        }),

    /** Forgets every photograph's faces and every choice made among them, which is what closing all of them leaves. */
    forgetAllFaces: () => set({ faces: new Map<string, Detected>(), choices: new Map<string, FaceChoice>() }),
}));

/**
 * The faces found in one photograph **at the framing in force**, or `undefined` where they are not known -
 * including where they were found at another framing.
 *
 * The framing is compared **by value**, through `sameCrop`, as `useEnhancementRun` compares the one its result was
 * made at.
 *
 * A photograph with no identity can only ever read as unknown: nothing can serve its pixels, so there is
 * nothing to detect in.
 */
export const useImageFaces = (identity: string | undefined, crop: CropInfo | undefined): Face[] | undefined =>
    useFacesStore((state) => {
        if (!identity) return;

        const detected = state.faces.get(identity);

        // A miss rather than a wrong answer is the whole point: a flip, a turn or a cut moves every face and can
        // add or remove one, so an answer found at another framing describes a photograph nobody is looking at.
        return detected && sameCrop(detected.crop, crop) ? detected.faces : undefined;
    });

// Same reference until a write, because `Map.get` answers the object the writer put in, and the writer replaces it
// whole - so two renders without a write are the same object, and a render after one never is.
//
// Not compared against the framing because an exception names a face rather than a framing, so one recorded at a
// framing nobody is at simply matches nothing: the miss is the answer, and checking the framing here would be a
// second statement of the same rule that could come to disagree.
/**
 * The choice made among one photograph's faces, or `undefined` where nobody has made one.
 *
 * **The same reference until a write happens**, which is what `useEnhancementRun` is built on: the choice is
 * among that effect's dependencies, so the chain re-runs exactly when a choice is applied and at no other time.
 *
 * A photograph with no identity has nothing keyed by it and can only ever read as no choice.
 */
export const useFaceChoice = (identity: string | undefined): FaceChoice | undefined =>
    useFacesStore((state) => (identity ? state.choices.get(identity) : undefined));

/* A per-file owner keyed by identity, exactly as the transform store's registration is. */
registerFileOwner({
    forget: (_path, identity) => {
        if (identity) useFacesStore.getState().forgetFaces(identity);
    },
    forgetAll: () => useFacesStore.getState().forgetAllFaces(),
});
