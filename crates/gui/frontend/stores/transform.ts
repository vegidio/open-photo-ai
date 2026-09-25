import { create } from "zustand";
import { ZOOM_MAX, ZOOM_MIN } from "@/lib/constants";
import { registerFileOwner } from "@/stores/files";

/**
 * How closely one photograph is being looked at, and which part of it is in view.
 *
 * The position is the offset of the photograph's top-left corner from the pane's, in pane pixels -
 * the coordinate system `features/preview/geometry.ts` documents and every number here is written
 * in.
 */
export type ImageTransform = {
    /** The magnification, between `ZOOM_MIN` and `ZOOM_MAX`, where 1 is the photograph drawn whole. */
    scale: number;
    x: number;
    y: number;
    // Optional so the pane holds either point with one branch, not two code paths.
    /**
     * The point of the displayed photograph to hold still while the scale changes, as fractions in
     * [0, 1].
     *
     * Written by the wheel - the point under the pointer - and left out by the slider and its two
     * step buttons, which are not pointed at any part of the photograph and so hold the middle of
     * the pane instead.
     */
    anchor?: { x: number; y: number };
};

/** The portion of the photograph the canvas is showing, as fractions in [0, 1] of the whole of it. */
export type ImageViewport = {
    x: number;
    y: number;
    width: number;
    height: number;
};

/** What an image that has never been magnified reads as: drawn whole, and centred by `constrain`. */
export const FITTED: ImageTransform = { scale: ZOOM_MIN, x: 0, y: 0 };

/**
 * A scale held to the range the preview allows.
 *
 * {@link TransformStore.setTransform} applies this to everything written, so no caller has to.
 */
export const clampScale = (scale: number) => Math.min(Math.max(scale, ZOOM_MIN), ZOOM_MAX);

type TransformStore = {
    // **Keyed by identity where the selection is keyed by path**, and the two are different on
    // purpose. `ipc/images.ts` calls identity "the key everything about this photograph hangs off".
    // The argument that made the selection path-keyed - identity is optional, so a file whose bytes
    // could not be read would have no key - does not apply, because such a file has no rendition URL
    // either and therefore cannot be drawn at all, let alone magnified. The transform is keyed the
    // same way the pixels are.
    //
    // Two copies of one photograph in different folders are two rows in the strip and two
    // independently pickable files, and one thing to look at; the reference collapses them the same
    // way, on `file.Hash`.
    /**
     * One transform per photograph, keyed by identity, so two copies of one photograph in different
     * folders share one view.
     */
    transforms: Map<string, ImageTransform>;
    // One slot rather than one per pane, because both panes always show the same part of the
    // photograph - which is the whole of what makes the comparison a comparison.
    /** What the sidebar's miniature draws its rectangle from, or absent before anything has drawn. */
    viewport?: ImageViewport;

    setTransform: (identity: string, transform: ImageTransform) => void;
    forgetTransform: (identity: string) => void;
    forgetAllTransforms: () => void;
    setViewport: (viewport: ImageViewport) => void;
};

// **Its own store, not the drawer's**, where the reference pairs `open` with `zoom`. That pairing is
// an artefact of both controls sitting in one header: how far into a photograph the user has gone is
// a property of that photograph and outlives the drawer being folded, and whether a strip is showing
// is not a property of any photograph.
//
// **Not the file store either.** That store holds what is open, which one is current and which are
// picked - facts about the *set* of files. A transform is a fact about one photograph, written sixty
// times a second during a drag, and putting it there would re-run every drawer item's selector on
// every frame of a pan.
/**
 * How each open photograph is being looked at, and which part of the current one is on the canvas.
 *
 * Not persisted. An entry is removed only when its photograph is closed, so within a session the map
 * grows with what is open and shrinks with what is closed. Each entry is four numbers.
 */
export const useTransformStore = create<TransformStore>()((set) => ({
    transforms: new Map<string, ImageTransform>(),

    /**
     * Records how one photograph is being looked at.
     *
     * The scale is clamped into range here. The position is *not* constrained here: it is constrained
     * against the pane's measured size, which this store cannot see, so every writer constrains
     * before it writes.
     */
    setTransform: (identity: string, transform: ImageTransform) =>
        set((state) => {
            // **Every write replaces the map rather than mutating it**, which is the standing rule for
            // every store here and the reason `immer` is absent where the reference has it: a Map
            // mutated in place is the same object, so zustand's equality check sees no write at all.
            // The cost is one `new Map(...)` per frame of a pan - a few hundred pointer-sized copies,
            // against a sixteen-millisecond budget - and the alternative, mutating in place and relying
            // on each subscriber's selector reading a fresh *value* out of the same map, happens to work
            // and is the quietest bug a store can ship: every assertion about values passes on it. It is
            // not worth saving a Map copy to have two rules about mutation in one codebase.
            const transforms = new Map(state.transforms);
            // Clamped here rather than at each of the four call sites - the wheel, the slider, the two
            // step buttons - because "there is no zooming out past the whole photograph and none in
            // past eight times" is a property of the value, not of the route that produced it. The pane
            // constrains the position again as it draws, so a transform stored against one pane size is
            // still legal when the window is resized under it.
            transforms.set(identity, {
                ...transform,
                scale: clampScale(transform.scale),
            });

            return { transforms };
        }),

    /**
     * Forgets how one photograph was being looked at.
     *
     * Called when its file is closed, which is the whole of what "a closed image's view is forgotten"
     * means: re-opening the file finds no entry and {@link useImageTransform} answers {@link FITTED}.
     *
     * Keyed by identity, so two copies of one photograph in different folders share one entry and
     * closing either forgets it for both; the surviving copy is drawn fitted.
     *
     * A no-op for a file with no identity, and for one that was never magnified: neither has an entry.
     */
    forgetTransform: (identity: string) =>
        set((state) => {
            // As the reference's `removeImageTransform` does. Forgetting both copies is right: they are
            // one thing to look at, and fitted is what an image the user has not magnified looks like
            // anyway. The map is replaced even when nothing was deleted, which costs one copy on a path
            // taken once per close and keeps the one rule this store has about mutation.
            const transforms = new Map(state.transforms);
            transforms.delete(identity);

            return { transforms };
        }),

    /** Forgets every photograph's view, which is what closing all of them leaves behind. */
    forgetAllTransforms: () => set({ transforms: new Map<string, ImageTransform>() }),

    // The reference has both panes write this slot on every animation frame of a pan and then compares
    // the value before storing it, to undo the re-render storm that causes; removing the second writer
    // removes the reason for the comparison.
    /**
     * Records which part of the current photograph is on the canvas.
     *
     * Written by one pane - the enhanced one, which is the only one drawn in all three comparisons -
     * so there is no comparison before the store.
     */
    setViewport: (viewport: ImageViewport) => set({ viewport }),
}));

// One accessor rather than every reader spelling out the identity lookup and the fallback, as the
// reference's `useImageTransform` is.
//
// Takes the identity rather than reading the current file, because the canvas draws the *settled*
// photograph - see `hooks/useSettledFile.ts` - and reading the current one there would apply the
// incoming image's magnification to the outgoing image's pixels for as long as the swap takes.
/**
 * How one photograph is being looked at, fitted if it has never been magnified.
 *
 * A photograph with no identity reads as fitted too, and it can only ever read as fitted: nothing
 * can serve its pixels, so there is nothing to magnify.
 */
export const useImageTransform = (identity?: string): ImageTransform =>
    useTransformStore((state) => (identity ? state.transforms.get(identity) : undefined)) ?? FITTED;

/*
 * Registered as a per-file owner - see `FileOwner` for why, and for why the file store hands over
 * both keys. Keyed by identity, so a file whose bytes could not be read has nothing here to forget,
 * and the path is ignored.
 */
registerFileOwner({
    forget: (_path, identity) => {
        if (identity) useTransformStore.getState().forgetTransform(identity);
    },
    forgetAll: () => useTransformStore.getState().forgetAllTransforms(),
});
