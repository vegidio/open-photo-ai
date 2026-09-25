import { create } from "zustand";
import type { CropInfo } from "@/ipc/crop";
import { registerFileOwner } from "@/stores/files";

type CropStore = {
    // Keyed by identity for the reasons `transforms` in `stores/transform.ts` gives, which apply here
    // word for word: a file whose bytes could not be read has no rendition to frame in the first place.
    // The reference keys its crop store by path.
    /**
     * One framing per photograph, keyed by identity, so two copies of one photograph in different
     * folders share one framing.
     */
    crops: Map<string, CropInfo>;

    setCrop: (identity: string, crop: CropInfo) => void;
    forgetCrop: (identity: string) => void;
    forgetAllCrops: () => void;
};

// Not persisted, as in the reference: a framing that outlived a restart would be a photograph the
// user opens and does not recognise. Each entry is four numbers and three flags.
/**
 * How each open photograph is framed. Written by the Crop/Rotate dialog.
 *
 * **Not persisted.** An entry is removed when its photograph is closed.
 */
export const useCropStore = create<CropStore>()((set) => ({
    crops: new Map<string, CropInfo>(),

    /** Records how one photograph is framed. */
    setCrop: (identity: string, crop: CropInfo) =>
        set((state) => {
            // Replaced rather than mutated: see `setTransform` in `stores/transform.ts`.
            const crops = new Map(state.crops);
            crops.set(identity, crop);

            return { crops };
        }),

    /**
     * Forgets how one photograph was framed, which is what closing it means.
     *
     * A no-op for a file with no identity and for one that was never framed: neither has an entry.
     */
    forgetCrop: (identity: string) =>
        set((state) => {
            // Replaced even when nothing was deleted, as `forgetTransform` does and for its reason.
            const crops = new Map(state.crops);
            crops.delete(identity);

            return { crops };
        }),

    /** Forgets every photograph's framing, which is what closing all of them leaves behind. */
    forgetAllCrops: () => set({ crops: new Map<string, CropInfo>() }),
}));

// One accessor taking the identity rather than reading the current file, for the reasons
// `useImageTransform` gives.
/**
 * How one photograph is framed, or `undefined` where it is drawn whole.
 *
 * A photograph with no identity reads as unframed and can only ever read as unframed: nothing can
 * serve its pixels, so there is nothing to frame.
 */
export const useImageCrop = (identity?: string): CropInfo | undefined =>
    useCropStore((state) => (identity ? state.crops.get(identity) : undefined));

/* A per-file owner keyed by identity, exactly as the transform store's registration is. */
registerFileOwner({
    forget: (_path, identity) => {
        if (identity) useCropStore.getState().forgetCrop(identity);
    },
    forgetAll: () => useCropStore.getState().forgetAllCrops(),
});
