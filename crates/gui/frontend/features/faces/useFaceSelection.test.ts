import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useFacesStore } from "@/stores/faces";
import { HOLIDAY } from "@/test/support";
import { useFaceSelection } from "./useFaceSelection";

const identity = HOLIDAY.identity ?? "";

const face = (left: number): Face => ({
    bounding_box: { min: { x: left, y: 4 }, max: { x: left + 3, y: 7 } },
    landmarks: [
        { x: left + 1, y: 5 },
        { x: left + 2, y: 5 },
        { x: left + 1.5, y: 6 },
        { x: left + 1, y: 6.5 },
        { x: left + 2, y: 6.5 },
    ],
    confidence: 0.9,
});

const FIRST = face(0);
const SECOND = face(20);
const THIRD = face(40);

/** What the store holds for the photograph, which is what only Apply is allowed to change. */
const stored = () => useFacesStore.getState().skipped.get(identity);

/** The dialog, opened over `who`. `open` is re-rendered so a test can close and re-open it. */
const mountOver = (who: string | undefined) =>
    renderHook(({ open }) => useFaceSelection(who, open), { initialProps: { open: true } });

/** The dialog, opened over the photograph every test but one is about. */
const mount = () => mountOver(identity);

beforeEach(() => {
    useFacesStore.setState(useFacesStore.getInitialState(), true);
});

describe("the choice the Select faces dialog is editing", () => {
    it("starts from nothing for a photograph nothing has been skipped in", () => {
        const { result } = mount();

        expect(result.current.skipped).toEqual(new Set());
    });

    it("seeds the working copy from what is committed", () => {
        act(() => useFacesStore.getState().setSkippedFaces(identity, new Set([faceKey(FIRST)])));

        const { result } = mount();

        expect(result.current.skipped).toEqual(new Set([faceKey(FIRST)]));
    });

    it("turns a face off and on again without touching the store", () => {
        const { result } = mount();

        act(() => result.current.toggle(FIRST));
        expect(result.current.skipped).toEqual(new Set([faceKey(FIRST)]));
        expect(stored()).toBeUndefined();

        act(() => result.current.toggle(FIRST));
        expect(result.current.skipped).toEqual(new Set());
        expect(stored()).toBeUndefined();
    });

    it("commits the working copy when it is applied", () => {
        const { result } = mount();

        act(() => result.current.toggle(SECOND));
        act(() => result.current.apply());

        expect(stored()).toEqual(new Set([faceKey(SECOND)]));
    });

    it("commits several toggles as one write", () => {
        // What the working copy is for: a write per box would be an inference run per box.
        const { result } = mount();

        act(() => {
            result.current.toggle(FIRST);
        });
        act(() => {
            result.current.toggle(SECOND);
        });
        act(() => {
            result.current.toggle(THIRD);
        });

        const before = useFacesStore.getState().skipped;
        act(() => result.current.apply());

        expect(useFacesStore.getState().skipped).not.toBe(before);
        expect(stored()).toEqual(new Set([FIRST, SECOND, THIRD].map(faceKey)));
    });

    it("writes nothing when what is applied is what was already committed", () => {
        // Opening the dialog to read the selection and applying it must leave the run in flight alone.
        act(() => useFacesStore.getState().setSkippedFaces(identity, new Set([faceKey(FIRST)])));

        const { result } = mount();
        const before = useFacesStore.getState().skipped;

        act(() => result.current.apply());

        expect(useFacesStore.getState().skipped).toBe(before);
    });

    it("writes nothing when a face is turned off and on again before applying", () => {
        const { result } = mount();

        act(() => result.current.toggle(FIRST));
        act(() => result.current.toggle(FIRST));

        const before = useFacesStore.getState().skipped;
        act(() => result.current.apply());

        expect(useFacesStore.getState().skipped).toBe(before);
    });

    it("leaves the store as it was when the dialog is dismissed rather than applied", () => {
        const { result, rerender } = mount();

        act(() => result.current.toggle(FIRST));
        act(() => result.current.toggle(SECOND));
        rerender({ open: false });

        expect(stored()).toBeUndefined();
    });

    it("re-seeds on every open, so a discarded edit does not reappear", () => {
        const { result, rerender } = mount();

        act(() => result.current.toggle(FIRST));
        rerender({ open: false });
        rerender({ open: true });

        expect(result.current.skipped).toEqual(new Set());
    });

    it("re-seeds from a choice committed while it was closed", () => {
        const { result, rerender } = mount();

        rerender({ open: false });
        act(() => useFacesStore.getState().setSkippedFaces(identity, new Set([faceKey(THIRD)])));
        rerender({ open: true });

        expect(result.current.skipped).toEqual(new Set([faceKey(THIRD)]));
    });

    it("writes nothing for a photograph whose bytes could not be read", () => {
        // No identity means no pixels to serve and none to detect in, so nothing to choose among.
        const { result } = mountOver(undefined);

        act(() => result.current.toggle(FIRST));
        act(() => result.current.apply());

        expect(useFacesStore.getState().skipped.size).toBe(0);
    });
});
