import { create } from "zustand";

type DrawerStore = {
    /** Whether the drawer's body - the strip of open images - is showing. */
    open: boolean;

    setOpen: (open: boolean) => void;
    toggle: () => void;
};

// A store rather than `useState` in `Drawer`, because three things outside the drawer's own subtree
// read or write it: the toaster clears the strip, the effect that unfolds it watches the open image
// count, and the fold toggle lives in `DrawerHeader` rather than in `Drawer`. Lifting it into `App`
// would put a piece of drawer state in the shell and thread it through two components to reach the
// toaster, which is mounted in `providers.tsx` and is not under `App` at all.
//
// **The zoom is not in it**, where the reference's `useDrawerStore` holds `open` and `zoom`
// together: it belongs with the per-image transform - see `stores/transform.ts`.
//
// Not persisted because whether a drawer is folded is a property of a session - the reference does
// not persist it either - and the window comes up folded because it comes up with nothing open.
/**
 * Whether the drawer is unfolded, and nothing else.
 *
 * Not persisted, and folded at start.
 */
export const useDrawerStore = create<DrawerStore>()((set) => ({
    open: false,

    setOpen: (open: boolean) => set({ open }),

    toggle: () => set((state) => ({ open: !state.open })),
}));
