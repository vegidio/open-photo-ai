import { create } from "zustand";
import type { ImageRecord } from "@/ipc/images";

// **Registered rather than named here.** Calling each per-file store's remover by hand would make this
// module import every one of them, and make "everything keyed by a file forgets it" a list that every
// new store has to be added to. A missed line leaks across a re-open at the same path, and nothing
// fails: the store simply answers with the previous file's state.
//
// Both keys are handed over because the stores disagree about which one they use - the transform is
// keyed by identity (the only honest key for pixels), the enhancement stack by path (a file whose
// bytes could not be read has no identity and is still an open image with a stack of its own).
//
// A store that has never been imported never registers - and holds nothing to forget, because
// nothing has written it. The two facts are the same fact, which is what makes this safe.
/**
 * Something holding state per open file, which has to let go of it when that file closes.
 *
 * An owner is handed both of a file's keys, takes the one it keys by and ignores the other.
 */
export type FileOwner = {
    /** Forgets what is held for one file, by whichever key this owner uses. `identity` is absent for
     * a file whose bytes could not be read. */
    forget: (path: string, identity?: string) => void;
    /** Forgets everything, which is what closing every image leaves behind. */
    forgetAll: () => void;
};

const owners = new Set<FileOwner>();

/** Registers `owner` for the life of the process. Called at module scope by each per-file store. */
export const registerFileOwner = (owner: FileOwner) => {
    owners.add(owner);
};

type FileStore = {
    /** The images the user has opened, in the order they were opened. */
    files: ImageRecord[];
    // An index rather than a record, as the reference keeps it: the drawer's strip is a list with a
    // position in it, and "which one" answered by identity would have to be searched for on every
    // read.
    /**
     * Which of {@link files} the rest of the window reports and draws.
     *
     * It is 0 while nothing is open, which addresses nothing - {@link useCurrentFile} answers
     * `undefined` for an empty list - so there is no separate "none" to check for.
     */
    currentIndex: number;
    // **One structure, not the reference's two.** It keeps `selectedFiles: File[]` *and* a Set of
    // their paths, its own comment saying the Set exists "purely so a membership test is O(1)":
    // every drawer item subscribes with its own "am I picked?" check, and scanning the array made
    // one Select all cost a quarter of a million comparisons at 500 files. The array is the half
    // with no second reason to exist - the only consumer that wants picked *records* is the export
    // queue, and it wants them once, when it opens, where `files.filter((file) =>
    // selectedPaths.has(file.path))` is one pass over an array already in hand. Keeping both means
    // every writer updating both, which is a class of bug for a pass performed once per export.
    //
    // **Keyed by path, not identity.** Identity is optional on a record - a file whose bytes could
    // not be read has none - and `addFiles` already de-duplicates by path, so paths are exactly the
    // unique keys of the list. Two copies of one photograph in different folders are two rows in
    // the strip and two independently pickable files, which identity would have collapsed into one.
    /** The paths of the images the user has picked, which is the whole of what the selection is. */
    selectedPaths: Set<string>;

    setCurrentIndex: (index: number) => void;
    addFiles: (files: ImageRecord[]) => void;
    closeFile: (path: string) => void;
    closeAll: () => void;
    toggleSelected: (path: string) => void;
    selectAll: () => void;
    unselectAll: () => void;
};

/**
 * Which row is current once the row at `removed` has been closed:
 *
 * ```
 *   nothing left afterwards         -> 0
 *   the closed row was below it     -> one less  (the rows below shifted down)
 *   the index has outrun the list   -> the last  (the closed row was the last one)
 *   otherwise                       -> unchanged
 * ```
 *
 * `remaining` is the length **after** the removal, which is what makes the third arm read as it does:
 * an index that has outrun the shortened list was pointing at the row that has just gone.
 */
const currentAfterClosing = (removed: number, current: number, remaining: number) => {
    // The reference's fixup. *Unchanged* is the non-obvious arm: it is what makes closing the current
    // image move **forward**, since the slot keeps its number and the photograph that followed has
    // slid into it. Closing repeatedly therefore walks through what is open rather than snapping back
    // to the first photograph each time.
    if (remaining === 0) return 0;
    if (removed < current) return current - 1;
    if (current >= remaining) return remaining - 1;

    return current;
};

// Every writer replaces the array or the Set rather than mutating it: see `setTransform` in
// `stores/transform.ts`.
//
// `closeFile` and `closeAll` being the only removers means the selection never has to survive a file
// disappearing from under it by any other route.
//
// Not persisted because what is open is a property of a session - the reference does not restore it
// either - and the records carry identities computed over bytes that may have changed since.
/**
 * What is open, which one of them the window is showing, and which of them the user has picked.
 *
 * **Closing is where a file leaves, and it takes everything keyed by that file with it.**
 * {@link FileStore.closeFile} and {@link FileStore.closeAll} are the only removers.
 *
 * Not persisted.
 */
export const useFileStore = create<FileStore>()((set) => ({
    files: [],
    currentIndex: 0,
    selectedPaths: new Set<string>(),

    // Choosing is a change of what the window is looking at rather than of what it holds.
    /**
     * Makes one of the open images the current one.
     *
     * Written by the drawer's strip, which is the only thing that offers the choice. It changes
     * nothing else - not the list, not its order, not what is picked.
     */
    setCurrentIndex: (index: number) => set({ currentIndex: index }),

    /**
     * Appends what is new and makes the first of it current.
     *
     * De-duplicated by path against what is already open **and within the batch itself**. A batch
     * that adds nothing new leaves the list, the current image and the selection alone, so opening
     * the same folder twice does not throw the canvas back to its first photograph.
     *
     * **A batch opened into an empty window picks its first file.** A batch arriving into a window
     * that already had images picks nothing.
     */
    addFiles: (files: ImageRecord[]) =>
        set((state) => {
            // The reference's rules. Within the batch because a drop can carry the same file twice. The
            // Set is built once per call rather than scanning the list per incoming file, which is the
            // reference's fix for a folder dropped onto a long list costing O(n·m).
            const seen = new Set(state.files.map((file) => file.path));
            const added = files.filter((file) => {
                if (seen.has(file.path)) return false;

                seen.add(file.path);
                return true;
            });

            // The subtle rule: this guard is what keeps re-opening an already-open folder from moving
            // the canvas.
            if (added.length === 0) return state;

            const next = {
                files: [...state.files, ...added],
                currentIndex: state.files.length,
            };

            // Picking the first file of a batch opened into an empty window makes a window with one
            // photograph in it ready to act on it. Otherwise the selection is a choice the user has
            // made, and files turning up is not a reason to revise it.
            //
            // `added[0]` rather than `files[0]`: they are the same on the batch that finds the window
            // empty, and reading the de-duplicated list is what keeps them the same if that ever
            // stops being true.
            const first = added[0];
            if (state.files.length > 0 || !first) return next;

            return { ...next, selectedPaths: new Set([first.path]) };
        }),

    /**
     * Closes one open image, which removes it from the window entirely: the row leaves, the selection
     * loses it and every registered {@link FileOwner} forgets it - everything keyed by the file, in
     * one action.
     *
     * The current image lands where `currentAfterClosing` puts it, so closing the current image moves
     * **forward** to the photograph that followed it.
     *
     * A path that is not open is a no-op.
     */
    closeFile: (path: string) =>
        set((state) => {
            // A no-op rather than a fixup applied to a row that was never there - which is what
            // `indexOf` returning -1 would otherwise produce.
            const removed = state.files.findIndex((file) => file.path === path);
            if (removed < 0) return state;

            // The identity is resolved from the record before it goes, for the owners keyed by it.
            const closed = state.files[removed];
            const files = state.files.toSpliced(removed, 1);

            // Everything keyed by the file goes in one action because a window that had closed an image
            // while still holding its zoom would apply that zoom to the next file admitted at the same
            // identity, and one still holding its stack would apply that stack to the next file opened
            // at the same path.
            //
            // Before the store is written rather than after: these are different stores, and leaving
            // the writes in one synchronous block is what keeps a subscriber from ever observing a
            // closed file's transform or enhancements still in their maps.
            for (const owner of owners) owner.forget(path, closed?.identity);

            const selectedPaths = new Set(state.selectedPaths);
            selectedPaths.delete(path);

            return {
                files,
                currentIndex: currentAfterClosing(removed, state.currentIndex, files.length),
                selectedPaths,
            };
        }),

    /**
     * Closes every open image, leaving the window holding none.
     *
     * `currentIndex` goes back to 0, which addresses nothing while the list is empty - see its own
     * documentation. Every registered {@link FileOwner} forgets everything with it.
     */
    closeAll: () => {
        // Offered alongside closing one rather than instead of it: a user who has dropped the wrong
        // folder wants the window emptied, not twenty individual closes.
        for (const owner of owners) owner.forgetAll();

        set({ files: [], currentIndex: 0, selectedPaths: new Set<string>() });
    },

    /** Picks an open image that was not picked, or unpicks one that was. */
    toggleSelected: (path: string) =>
        set((state) => {
            // One writer for both directions rather than the reference's `addSelectedFile` and
            // `removeSelectedFile`: a checkbox reports the state it has arrived at, and the store
            // already knows which of the two that is.
            const selectedPaths = new Set(state.selectedPaths);

            if (!selectedPaths.delete(path)) selectedPaths.add(path);

            return { selectedPaths };
        }),

    /** Picks every open image. */
    selectAll: () => set((state) => ({ selectedPaths: new Set(state.files.map((file) => file.path)) })),

    /** Unpicks every open image. */
    unselectAll: () => set({ selectedPaths: new Set<string>() }),
}));

// One accessor rather than four regions each writing `files[currentIndex]`, exactly as the reference
// does it: the canvas, the navbar, the sidebar and the drawer all ask the same question, and it
// should have one answer.
/**
 * The image the window is reporting and drawing, or `undefined` while none is open.
 *
 * `at` rather than an index read, so an index that has outrun the list answers `undefined` rather
 * than throwing on a property of nothing.
 */
export const useCurrentFile = () => useFileStore((state) => state.files.at(state.currentIndex));

// Beside `useCurrentFile` rather than computed in the shell and passed down, so "no image is open" is
// one sentence in the store that knows rather than a prop threaded through the regions that do not.
/**
 * Whether any image is open, which is what every control that acts on one is gated on.
 *
 * The boolean rather than the list, so a subscriber re-renders when the window goes from empty to not
 * and not on every image added after that.
 */
export const useHasFiles = () => useFileStore((state) => state.files.length > 0);
