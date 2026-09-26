import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { Face } from "@/ipc/faces";
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
    restorable: true,
    key: `${left},4,${left + 3},7`,
});

const FIRST = face(0);
const SECOND = face(20);
const THIRD = face(40);

/** A face too large to restore, which the default leaves alone. */
const LARGE: Face = { ...face(60), restorable: false, key: "60,4,63,7" };

const FACES = [FIRST, SECOND, THIRD, LARGE];

/** What the store holds for the photograph, which is what only Apply is allowed to change. */
const stored = () => useFacesStore.getState().choices.get(identity);

const choose = (skipped: Face[], restored: Face[] = []) =>
    act(() =>
        useFacesStore.getState().setFaceChoice(identity, {
            skipped: skipped.map((face) => face.key),
            restored: restored.map((face) => face.key),
        }),
    );

/** The dialog, opened over `who`. `open` is re-rendered so a test can close and re-open it. */
const mountOver = (who: string | undefined) =>
    renderHook(({ open }) => useFaceSelection(who, FACES, open), { initialProps: { open: true } });

/** The dialog, opened over the photograph every test but one is about. */
const mount = () => mountOver(identity);

beforeEach(() => {
    useFacesStore.setState(useFacesStore.getInitialState(), true);
});

describe("the choice the Select faces dialog is editing", () => {
    it("starts from each face's default for a photograph nobody has chosen in", () => {
        const { result } = mount();

        // Every restorable face chosen, and the one too large to restore left alone.
        expect(result.current.skipped).toEqual(new Set([LARGE.key]));
    });

    it("seeds the working copy from what is committed", () => {
        choose([FIRST], [LARGE]);

        const { result } = mount();

        expect(result.current.skipped).toEqual(new Set([FIRST.key]));
    });

    it("turns a face off and on again without touching the store", () => {
        const { result } = mount();

        act(() => result.current.toggle(FIRST));
        expect(result.current.skipped).toEqual(new Set([LARGE.key, FIRST.key]));
        expect(stored()).toBeUndefined();

        act(() => result.current.toggle(FIRST));
        expect(result.current.skipped).toEqual(new Set([LARGE.key]));
        expect(stored()).toBeUndefined();
    });

    it("commits only the exceptions to each face's default when it is applied", () => {
        const { result } = mount();

        act(() => result.current.toggle(SECOND));
        act(() => result.current.toggle(LARGE));
        act(() => result.current.apply());

        // A restorable face turned off, and a large one turned on: the two things the default would not do.
        expect(stored()).toEqual({ skipped: [SECOND.key], restored: [LARGE.key] });
    });

    it("commits several toggles as one write", () => {
        // What the working copy is for: a write per box would be an inference run per box.
        const { result } = mount();

        act(() => result.current.toggle(FIRST));
        act(() => result.current.toggle(SECOND));
        act(() => result.current.toggle(THIRD));

        const before = useFacesStore.getState().choices;
        act(() => result.current.apply());

        expect(useFacesStore.getState().choices).not.toBe(before);
        expect(stored()).toEqual({ skipped: [FIRST, SECOND, THIRD].map((face) => face.key), restored: [] });
    });

    it("writes nothing when what is applied is what was already committed", () => {
        // Opening the dialog to read the selection and applying it must leave the run in flight alone.
        choose([FIRST]);

        const { result } = mount();
        const before = useFacesStore.getState().choices;

        act(() => result.current.apply());

        expect(useFacesStore.getState().choices).toBe(before);
    });

    it("writes nothing when the faces are left at their defaults", () => {
        const { result } = mount();

        act(() => result.current.toggle(FIRST));
        act(() => result.current.toggle(FIRST));

        const before = useFacesStore.getState().choices;
        act(() => result.current.apply());

        expect(useFacesStore.getState().choices).toBe(before);
    });

    it("keeps what was chosen at another framing when this one is applied", () => {
        // A face at a framing the user is not at: its key matches none shown here, and a framing returned to must
        // find it where it was left.
        act(() => useFacesStore.getState().setFaceChoice(identity, { skipped: ["900,4,903,7"], restored: [] }));

        const { result } = mount();
        act(() => result.current.toggle(FIRST));
        act(() => result.current.apply());

        expect(stored()).toEqual({ skipped: ["900,4,903,7", FIRST.key], restored: [] });
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

        expect(result.current.skipped).toEqual(new Set([LARGE.key]));
    });

    it("re-seeds from a choice committed while it was closed", () => {
        const { result, rerender } = mount();

        rerender({ open: false });
        choose([THIRD]);
        rerender({ open: true });

        expect(result.current.skipped).toEqual(new Set([THIRD.key, LARGE.key]));
    });

    it("writes nothing for a photograph whose bytes could not be read", () => {
        // No identity means no pixels to serve and none to detect in, so nothing to choose among.
        const { result } = mountOver(undefined);

        act(() => result.current.toggle(FIRST));
        act(() => result.current.apply());

        expect(useFacesStore.getState().choices.size).toBe(0);
    });
});
