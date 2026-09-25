import { beforeEach, describe, expect, it } from "vitest";
import { useDrawerStore } from "./drawer";

const state = () => useDrawerStore.getState();

describe("useDrawerStore", () => {
    beforeEach(() => {
        useDrawerStore.setState(useDrawerStore.getInitialState(), true);
    });

    it("comes up folded", () => {
        expect(state().open).toBe(false);
    });

    it("is unfolded and folded by the setter", () => {
        state().setOpen(true);
        expect(state().open).toBe(true);

        state().setOpen(false);
        expect(state().open).toBe(false);
    });

    it("flips with the toggle, from either state", () => {
        state().toggle();
        expect(state().open).toBe(true);

        state().toggle();
        expect(state().open).toBe(false);
    });

    /**
     * The zoom's absence, asserted rather than left to be noticed: it is kept out deliberately - see
     * `stores/transform.ts` - and a `zoom` quietly added here is the decision being reversed without
     * the argument being had.
     */
    it("holds the fold and nothing else", () => {
        expect(Object.keys(state()).sort()).toEqual(["open", "setOpen", "toggle"]);
    });

    it("does not write the fold to storage", async () => {
        state().setOpen(true);

        // `persist` writes on a microtask, so a store that had it would have written by here.
        await Promise.resolve();

        expect(localStorage.length).toBe(0);
    });
});
