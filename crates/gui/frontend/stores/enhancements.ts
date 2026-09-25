import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { Family } from "@/ipc/catalogue";
import { type Operation, releaseAllEnhanced, releaseEnhanced } from "@/ipc/enhance";
import { report } from "@/lib/report";
import { registerFileOwner } from "@/stores/files";

type EnhancementStore = {
    // Not gated on having a file open, deliberately: autopilot is a standing preference about what
    // should happen when an image is loaded, not an action on the image currently open, so it is the
    // one live control on the empty shell.
    /** Whether an image is analysed and enhancements suggested for it as soon as it is loaded. */
    autopilot: boolean;

    // **By path, not by the record.** A Map keyed on the `ImageRecord` object would use reference
    // identity, so any path that rebuilds a record for the same image - a re-drop, a re-describe -
    // would orphan that file's stack and leave the old entry behind forever. The reference's own
    // store carries the same comment for the same three Maps. The path is stable and unique within
    // the file list, which is also what `stores/files.ts` keys its selection on.
    //
    // **It holds the wire's own `Operation`** rather than a domain type of this side's. That type is
    // already exactly what an enhancement is - a family, a model, a precision and the family's own
    // values - so a parallel type would be a second spelling of the same four fields and the
    // conversion between them a place for the two to disagree.
    //
    // **Keyed by family within a stack** because the wire operation carries no identifier, and the
    // add menu's rule - an enhancement already in the stack cannot be added again - makes the family
    // unique within the list. The reference keys its rows on an operation id and its own comment says
    // what that id is for there: a stale id rehydrated from a previous version's persisted state.
    // Nothing here is persisted, so nothing stale can arrive.
    /**
     * What each open image is set to have done to it, keyed by the file's path.
     *
     * **One entry per family**, which is what {@link EnhancementStore.replaceEnhancement} and
     * {@link EnhancementStore.removeEnhancement} address.
     */
    enhancements: Map<string, Operation[]>;

    setAutopilot: (enable: boolean) => void;
    toggle: () => void;

    addEnhancement: (path: string, operation: Operation, order: readonly Family[]) => void;
    addEnhancements: (path: string, operations: Operation[], order: readonly Family[]) => void;
    replaceEnhancement: (path: string, operation: Operation) => void;
    removeEnhancement: (path: string, family: Operation["family"]) => void;

    removeKey: (path: string) => void;
    clear: () => void;
};

/**
 * `operations` in the order a chain applies them, which is `order` - the library's, read off the
 * catalogue by `applyOrder` in `lib/enhancements.ts`. A family `order` does not name sorts last, and
 * operations the order does not tell apart keep the order they were in.
 *
 * **The order is handed in by the writer** rather than read here, because it arrives with the catalogue
 * and both writers already hold it: the add menu builds its operation from the catalogue, and Autopilot
 * awaits it before building its batch. A store reading the catalogue itself would be a second, hidden
 * wait on the same answer.
 */
const inApplyOrder = (operations: Operation[], order: readonly Family[]) => {
    const position = (operation: Operation) => {
        const index = order.indexOf(operation.family);

        // Last rather than first: an unknown family cannot arrive from the menu, and putting an unknown
        // operation at the head of a chain would change what every operation after it sees.
        return index < 0 ? order.length : index;
    };

    return operations.sort((left, right) => position(left) - position(right));
};

// Every writer replaces the Map rather than mutating it: see `setTransform` in `stores/transform.ts`.
//
// A stack is replaced only when it changes because the run effect keys on the array itself, so a
// writer handing back a fresh array on every call would restart the run.
/**
 * Autopilot, and what each open image is set to have done to it.
 *
 * Only `autopilot` is persisted. **A stack is replaced only when it changes.**
 */
export const useEnhancementStore = create<EnhancementStore>()(
    persist(
        (set) => ({
            autopilot: true,
            enhancements: new Map<string, Operation[]>(),

            setAutopilot: (enable: boolean) => set({ autopilot: enable }),

            toggle: () => set((state) => ({ autopilot: !state.autopilot })),

            /** Appends an enhancement to one image's stack and puts the stack back in `order`, the chain's own. */
            addEnhancement: (path: string, operation: Operation, order: readonly Family[]) =>
                set((state) => {
                    // Ordered on the way in rather than on the way out, as the reference does it: the
                    // chain that is sent and the list that is drawn are the same array, so there is no
                    // second place for the two to disagree about what order the operations run in.
                    const next = inApplyOrder([...(state.enhancements.get(path) ?? []), operation], order);

                    return { enhancements: new Map(state.enhancements).set(path, next) };
                }),

            /**
             * Adds a batch of enhancements to one image's stack in **one write**, leaving out any family the stack
             * already carries, and puts the stack back in `order`, the chain's own.
             *
             * Autopilot's writer. One write rather than one {@link EnhancementStore.addEnhancement} per operation,
             * because the run effect keys on the stack array: N writes would start a run on the first suggestion,
             * cancel it on the second, and so on.
             *
             * **The key is written even for an empty batch**, which is what makes an analysis that answered
             * "nothing" mean "analysed": the photograph now has a list, so it is not analysed again.
             */
            addEnhancements: (path: string, operations: Operation[], order: readonly Family[]) =>
                set((state) => {
                    // An enhancement already there - one the user added by hand while the analysis ran - is kept
                    // as it is: the store holds one entry per family, and the user's is the later word.
                    const current = state.enhancements.get(path) ?? [];
                    const present = new Set(current.map((operation) => operation.family));
                    const next = inApplyOrder(
                        [...current, ...operations.filter((operation) => !present.has(operation.family))],
                        order,
                    );

                    return { enhancements: new Map(state.enhancements).set(path, next) };
                }),

            /**
             * Swaps the enhancement of that operation's family for the one given.
             *
             * Written by the options panel, which changes a model or a value on an enhancement that
             * is already in the stack. An operation of a family the stack does not carry leaves the
             * stack untouched rather than being appended.
             */
            replaceEnhancement: (path: string, operation: Operation) =>
                set((state) => {
                    // Never appended: the panel is only open over a row, so there is no state in which
                    // adding one would be the answer.
                    const current = state.enhancements.get(path);
                    if (!current) return state;

                    const next = current.map((existing) =>
                        existing.family === operation.family ? operation : existing,
                    );

                    return { enhancements: new Map(state.enhancements).set(path, next) };
                }),

            /** Drops one image's enhancement of that family, leaving the rest of its stack in order. */
            removeEnhancement: (path: string, family: Operation["family"]) =>
                set((state) => {
                    const current = state.enhancements.get(path);
                    if (!current) return state;

                    return {
                        enhancements: new Map(state.enhancements).set(
                            path,
                            current.filter((operation) => operation.family !== family),
                        ),
                    };
                }),

            /** Forgets everything one image was set to have done to it, because it has been closed. */
            removeKey: (path: string) =>
                set((state) => {
                    if (!state.enhancements.has(path)) return state;

                    const enhancements = new Map(state.enhancements);
                    enhancements.delete(path);

                    return { enhancements };
                }),

            /** Forgets every image's stack, because every image has been closed. */
            clear: () => set({ enhancements: new Map<string, Operation[]>() }),
        }),
        {
            // The Wails app's key, unchanged: a user upgrading from it keeps the setting they chose.
            name: "enhancements-storage",
            // `autopilot` alone, which is the Wails store's persisted surface exactly: that store holds
            // four fields, three of which are Maps of per-file state, and its `partialize` keeps only
            // `autopilot`. A stack describes the photograph on screen now, and the records it would be
            // keyed against are gone by the next launch.
            partialize: (state) => ({ autopilot: state.autopilot }),
        },
    ),
);

// A module constant rather than a `?? []` at the call site, which would build a fresh array on every
// render: the run effect keys on the array itself, so a new one per render would cancel and restart
// the run continuously. The same reason `stores/transform.ts` holds one identity transform.
/** The one empty stack every image without one is answered with. */
const NO_ENHANCEMENTS: Operation[] = [];

/**
 * What one image is set to have done to it, or the shared empty stack where it is set to have
 * nothing done to it.
 *
 * `undefined` for the path answers the empty stack too, so a window with nothing open reads the same
 * way as an image with no enhancements - which is what both of them mean to everything downstream.
 */
export const useFileEnhancements = (path: string | undefined) =>
    useEnhancementStore((state) =>
        path === undefined ? NO_ENHANCEMENTS : (state.enhancements.get(path) ?? NO_ENHANCEMENTS),
    );

/** A release that could not be made, which the user is deliberately not told about. */
const releaseFailed = (error: unknown) => {
    // Nothing has been lost that they can see: the pixels stay resident until the next run displaces
    // them. A dialog over a closed photograph would be a worse outcome than the memory it is about.
    report("an enhanced result could not be released", error);
};

/*
 * A per-file owner keyed by path, so the stack is forgotten unconditionally: a file with no identity
 * is still an open image with a stack of its own.
 *
 * The release is made from here too. The registry is what guarantees every holder of per-file state
 * is told a file has gone, and the enhanced pixels Rust is holding are per-file state like any other -
 * they are simply held on the other side of the boundary. Wiring the release into `closeFile`
 * directly would make it one more thing that has to be remembered there, which is the arrangement
 * the registry exists to avoid.
 *
 * The identity is the source photograph's, which is what the backend matches a release against - the
 * window closes a photograph, not a result. A file whose bytes could not be read has none, and has
 * produced nothing to release.
 */
registerFileOwner({
    forget: (path, identity) => {
        useEnhancementStore.getState().removeKey(path);

        if (identity !== undefined) void releaseEnhanced(identity).catch(releaseFailed);
    },
    forgetAll: () => {
        useEnhancementStore.getState().clear();

        void releaseAllEnhanced().catch(releaseFailed);
    },
});
