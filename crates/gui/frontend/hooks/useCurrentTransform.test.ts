import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useCurrentTransform } from "@/hooks/useCurrentTransform";
import { ZOOM_MIN } from "@/lib/constants";
import { useFileStore } from "@/stores/files";
import { FITTED, useTransformStore } from "@/stores/transform";
import { HOLIDAY, openFiles, resetFileStore, SUNSET } from "@/test/support";

const identity = (record: { identity?: string }) => record.identity ?? "";

const set = (record: { identity?: string }, transform: Parameters<typeof store.setTransform>[1]) =>
    act(() => store.setTransform(identity(record), transform));

const store = useTransformStore.getState();

beforeEach(() => {
    resetFileStore();
    useTransformStore.setState(useTransformStore.getInitialState(), true);
});

describe("the current image's transform", () => {
    it("reads as fitted for an image that has never been magnified", () => {
        openFiles(HOLIDAY);

        expect(renderHook(() => useCurrentTransform()).result.current).toEqual(FITTED);
        expect(FITTED.scale).toBe(ZOOM_MIN);
    });

    it("reads as fitted with nothing open at all", () => {
        expect(renderHook(() => useCurrentTransform()).result.current).toEqual(FITTED);
    });

    it("follows the image the window is drawing", () => {
        openFiles(HOLIDAY, SUNSET);
        set(HOLIDAY, { scale: 4, x: -100, y: -50 });

        // `addFiles` makes the first of the batch current, so this is the holiday photograph.
        const { result } = renderHook(() => useCurrentTransform());
        expect(result.current).toEqual({ scale: 4, x: -100, y: -50 });

        // Going to the second and back is the whole of what "returning to an image returns to what
        // was being examined" means, and neither image's view moved when the other's was written.
        act(() => useFileStore.getState().setCurrentIndex(1));
        expect(result.current).toEqual(FITTED);

        act(() => useFileStore.getState().setCurrentIndex(0));
        expect(result.current).toEqual({ scale: 4, x: -100, y: -50 });
    });

    it("reads as fitted for a file whose pixels nothing can serve", () => {
        // No identity, so nothing addresses it and nothing can magnify it. Built by leaving the key
        // out rather than setting it to `undefined`, which is not the same record here.
        const { identity: _, ...unreadable } = HOLIDAY;
        openFiles(unreadable);

        expect(renderHook(() => useCurrentTransform()).result.current).toEqual(FITTED);
    });
});
