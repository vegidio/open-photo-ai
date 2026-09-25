import { act } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ZOOM_MAX, ZOOM_MIN } from "@/lib/constants";
import { HOLIDAY, resetFileStore, SUNSET } from "@/test/support";
import { useTransformStore } from "./transform";

const identity = (record: { identity?: string }) => record.identity ?? "";

const set = (record: { identity?: string }, transform: Parameters<typeof store.setTransform>[1]) =>
    act(() => store.setTransform(identity(record), transform));

const store = useTransformStore.getState();
const transforms = () => useTransformStore.getState().transforms;

beforeEach(() => {
    resetFileStore();
    useTransformStore.setState(useTransformStore.getInitialState(), true);
});

describe("the transform store", () => {
    it("keeps one photograph's view separate from another's", () => {
        set(HOLIDAY, { scale: 4, x: -100, y: -50 });
        set(SUNSET, { scale: 2, x: -10, y: -20 });

        expect(transforms().get(identity(HOLIDAY))).toEqual({ scale: 4, x: -100, y: -50 });
        expect(transforms().get(identity(SUNSET))).toEqual({ scale: 2, x: -10, y: -20 });
    });

    it("replaces the map on every write rather than mutating it", () => {
        const before = transforms();

        set(HOLIDAY, { scale: 2, x: 0, y: 0 });

        // The standing rule, argued in `setTransform`.
        expect(transforms()).not.toBe(before);
        expect(before.size).toBe(0);
    });

    it("holds the scale inside the range whatever it is handed", () => {
        set(HOLIDAY, { scale: 0.25, x: 0, y: 0 });
        expect(transforms().get(identity(HOLIDAY))?.scale).toBe(ZOOM_MIN);

        set(HOLIDAY, { scale: 40, x: 0, y: 0 });
        expect(transforms().get(identity(HOLIDAY))?.scale).toBe(ZOOM_MAX);
    });

    it("keeps the anchor a writer handed it, and leaves it out when none was", () => {
        set(HOLIDAY, { scale: 2, x: 0, y: 0, anchor: { x: 0.25, y: 0.75 } });
        expect(transforms().get(identity(HOLIDAY))?.anchor).toEqual({ x: 0.25, y: 0.75 });

        set(HOLIDAY, { scale: 3, x: 0, y: 0 });
        expect(transforms().get(identity(HOLIDAY))).not.toHaveProperty("anchor");
    });
});
