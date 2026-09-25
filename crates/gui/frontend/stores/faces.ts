import { create } from "zustand";
import type { CropInfo } from "@/ipc/crop";
import type { Face } from "@/ipc/faces";
import { decidedSkips, withDecided } from "@/lib/faces";
import { registerFileOwner } from "@/stores/files";

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

    // **A skipped face is recorded by value rather than by its place among the faces found**, which is what makes
    // a framing change need no rule of its own: a flip, a turn or a cut moves every face, so nothing in the new
    // framing matches anything skipped in the old one, and every face in it is decided again by `setFaces` from
    // its size. A framing returned to is answered by the faces found there the first time - byte for byte, since
    // `opai`'s own run store serves it - so the keys match again and the choice is back in force. The reference
    // keeps indices instead and therefore carries a choice across a cut *by position*, which promotes whichever
    // face is now second into the slot the user skipped.
    //
    // **Not keyed by framing**, which follows from the by-value rule: a key from a framing nobody is at matches
    // nothing and costs one string. The set grows only by faces a person clicked and faces the detector found, so
    // it is bounded by the photograph.
    //
    // **Here rather than beside the enhancement stack**, because the choice is about the pixels and not about the
    // operation: it names faces, so it belongs where the faces are, and removing a face recovery and adding
    // another leaves it alone, which is a fact about where it is kept rather than an effect someone has to write.
    // It is *seeded* here as well, because the default among a set of faces is a function of the faces themselves.
    /**
     * The faces skipped in each photograph, keyed by identity: a set of the keys `lib/faces.ts` derives from a
     * face's bounding box.
     *
     * Written by the user's choice and by the size rule both: what it holds is which faces a recovery leaves
     * alone, however that came to be true.
     *
     * Absent for a photograph nothing has been skipped in, which is the same answer as an empty set and is the
     * one that costs nothing to hold.
     */
    skipped: Map<string, ReadonlySet<string>>;

    // **This is what "already decided" means**, and the reason it is a record of its own rather than something
    // read off `faces`: that map holds one detection, the one at the framing in force, so a face read against it
    // stops being decided the moment the photograph is framed another way. Cropping and undoing the crop would
    // then put a large face the user turned *on* back through the size rule and skip it again - undoing by itself
    // a choice the user made by hand, which `gui-faces` forbids in as many words. Accumulating instead means a
    // framing returned to is answered by keys that are all in here already, so nothing is decided twice.
    //
    // Never pruned for that same reason: a key dropped on the way out of a framing is a key missing on the way
    // back into it. It grows by one short string per face per framing visited - bounded by the photograph, like
    // the choice beside it.
    /**
     * Every face each photograph has been asked about, keyed by identity: the same face keys
     * {@link FacesStore.skipped} holds, for every face found at every framing visited since it was opened.
     *
     * **Never pruned while the photograph is open**, and released with the faces and the choice when it is
     * closed.
     *
     * Absent for a photograph nothing has been detected in, which is the same answer as an empty set.
     */
    decided: Map<string, ReadonlySet<string>>;

    setFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) => void;
    setSkippedFaces: (identity: string, skipped: ReadonlySet<string>) => void;
    forgetFaces: (identity: string) => void;
    forgetAllFaces: () => void;
};

// **The faces' writers are the run path and Autopilot**, and both write the same value through the same
// `setFaces`. `hooks/useEnhancementRun.ts` detects before enhancing when the stack carries a face recovery whose
// faces are not known; `hooks/useAutopilot.ts` detects when an analysis suggests face recovery, to decide whether
// to keep the suggestion - and records the answer so the run that follows does not detect again. Nothing else asks
// for a detection, which is what makes "faces are found only for a photograph whose enhancements need them, or
// that Autopilot is deciding about" structural.
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
export const useFacesStore = create<FacesStore>()((set) => ({
    faces: new Map<string, Detected>(),
    skipped: new Map<string, ReadonlySet<string>>(),
    decided: new Map<string, ReadonlySet<string>>(),

    /**
     * Records the faces found in one photograph at one framing, **and decides the default among the ones
     * nothing has decided about yet** - those not in {@link FacesStore.decided}: a face larger than
     * `lib/faces.ts`'s `RESTORED_FACE_AREA` is skipped, a face at or below it is chosen.
     *
     * An empty array is a legitimate value and is written like any other: it is what a photograph with nobody
     * in it answers, and what a failed detection is recorded as - so that "are this photograph's faces known?"
     * has an answer and a run is not held forever waiting for one.
     *
     * **Every map is written in one `set`**, and the choice and the decisions only where this actually changes
     * them, so a re-detection at the framing in force writes neither of them.
     */
    setFaces: (identity: string, crop: CropInfo | undefined, faces: Face[]) =>
        set((state) => {
            // One `set`, which is what makes this the writer rather than the caller. `useEnhancementRun` lists the
            // faces *and* the skipped set among its effect's dependencies, so two writes would give it one render
            // holding new faces beside the old choice - a chain started over every face found, cancelled by the
            // next render and started again.
            const next = new Map(state.faces);
            next.set(identity, { ...(crop && { crop }), faces });

            // Read off `state` **before** this detection is folded into it: the rule decides the faces nothing has
            // asked about yet, and a set that already held them would decide none of them.
            const seen = state.decided.get(identity);
            const committed = state.skipped.get(identity);

            const skips = decidedSkips(faces, seen, committed);
            const seenNow = withDecided(faces, seen);

            // Each set is written only where it changed, which both helpers report by handing back the set they
            // were given. A set replaced with an equal one is a new reference, and a new reference is exactly what
            // that effect reads as a choice the user just applied.
            return {
                faces: next,
                ...(seenNow && seenNow !== seen && { decided: new Map(state.decided).set(identity, seenNow) }),
                ...(skips && skips !== committed && { skipped: new Map(state.skipped).set(identity, skips) }),
            };
        }),

    /**
     * Records which of one photograph's faces the user has skipped.
     *
     * **The whole set is replaced on every write**, as the whole map is.
     *
     * An empty set is written like any other rather than deleted: "nothing is skipped here now" is a choice a
     * user just applied, and an entry that vanished would be indistinguishable from one that was never made.
     */
    setSkippedFaces: (identity: string, skipped: ReadonlySet<string>) =>
        set((state) => {
            // A new set as well as a new map, for the rule `setTransform` in `stores/transform.ts` states. Here a
            // subscriber comparing references is not only zustand's equality check - it is what
            // `useEnhancementRun` depends on to re-run the chain exactly once per applied choice and at no other
            // time, which a mutated set would defeat in the other direction by never re-running at all.
            //
            // An entry that vanished costs nothing today, and would cost a wrong answer the moment anything asked
            // whether this photograph had been looked at.
            const next = new Map(state.skipped);
            next.set(identity, new Set(skipped));

            return { skipped: next };
        }),

    /**
     * Forgets the faces found in one photograph, the choice made among them and which of them had been
     * decided, which is what closing it means. A photograph opened again is asked about afresh.
     *
     * A no-op for a file with no identity and for one nothing was ever detected in: neither has an entry.
     */
    forgetFaces: (identity: string) =>
        set((state) => {
            // All three go together, the decisions included: keys left behind would answer "already decided"
            // for faces nothing has decided this time round, and the size rule would leave a large face chosen
            // on a photograph nobody had chosen it in. Replaced even when nothing was deleted, as
            // `forgetTransform` does and for its reason.
            const faces = new Map(state.faces);
            faces.delete(identity);

            const skipped = new Map(state.skipped);
            skipped.delete(identity);

            const decided = new Map(state.decided);
            decided.delete(identity);

            return { faces, skipped, decided };
        }),

    /**
     * Forgets every photograph's faces, every choice made among them and every decision behind one, which
     * is what closing all of them leaves behind.
     */
    forgetAllFaces: () =>
        set({
            faces: new Map<string, Detected>(),
            skipped: new Map<string, ReadonlySet<string>>(),
            decided: new Map<string, ReadonlySet<string>>(),
        }),
}));

/**
 * The faces found in one photograph **at the framing in force**, or `undefined` where they are not known -
 * including where they were found at another framing. The caller reads that miss as "detect again".
 *
 * The framing is compared **by reference**, as `useEnhancementRun` compares the one its result was made at.
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
        // The miss is what makes a framing change re-detect and a framing returned to reuse what `opai`'s store
        // already holds.
        //
        // By reference is enough because the crop store replaces the whole value on a write and hands back the
        // same object until one happens, so two renders of one framing are the same reference and two framings
        // never are.
        return detected && detected.crop === crop ? detected.faces : undefined;
    });

// Same reference until a write, because `Map.get` answers the set the writer put in, and the writer replaces it
// whole - so two renders without a write are the same object, and a render after one never is.
//
// Not compared against the framing because a skipped key names a face rather than a framing, so one recorded at
// a framing nobody is at simply matches nothing: the miss is the answer, and checking the framing here would be a
// second statement of the same rule that could come to disagree.
/**
 * The faces the user has skipped in one photograph, or `undefined` where none have been.
 *
 * **The same reference until a write happens**, which is what `useEnhancementRun` is built on: the set is
 * among that effect's dependencies, so the chain re-runs exactly when a choice is applied and at no other
 * time.
 *
 * **Not compared against the framing**, unlike {@link useImageFaces}.
 *
 * A photograph with no identity has nothing keyed by it and can only ever read as nothing skipped.
 */
export const useSkippedFaces = (identity: string | undefined): ReadonlySet<string> | undefined =>
    useFacesStore((state) => (identity ? state.skipped.get(identity) : undefined));

/* A per-file owner keyed by identity, exactly as the transform store's registration is. */
registerFileOwner({
    forget: (_path, identity) => {
        if (identity) useFacesStore.getState().forgetFaces(identity);
    },
    forgetAll: () => useFacesStore.getState().forgetAllFaces(),
});
