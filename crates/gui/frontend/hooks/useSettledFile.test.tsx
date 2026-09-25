import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cropQuery } from "@/ipc/crop";
import { useFileStore } from "@/stores/files";
import { FRAMING, frame, HOLIDAY, openFiles, resetCropStore, resetFileStore, SUNSET } from "@/test/support";
import { useSettledFile } from "./useSettledFile";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of. Answered
// with the identity it was asked for, which is all this file needs of a rendition URL.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

/**
 * An `HTMLImageElement.decode` whose promises are settled on demand.
 *
 * jsdom implements no `decode` at all, and the hook's own feature test then makes every swap
 * immediate - which is the behaviour this exists to *replace*. Installing a controllable one is the
 * only way to hold a decode open and assert what the canvas is drawing while it is in flight.
 *
 * Defined rather than spied on, because `vi.spyOn` refuses a property that is not there - which is
 * also why the `afterEach` below takes it away again rather than leaving it to `restoreMocks`.
 */
const controllable = () => {
    const pending: { resolve: () => void; reject: () => void }[] = [];
    /** The `src` of every preload asked to decode, which is the URL the canvas is waiting on. */
    const requested: string[] = [];

    Object.defineProperty(HTMLImageElement.prototype, "decode", {
        configurable: true,
        writable: true,
        // A `function` rather than an arrow, so `this` is the preload element and the URL it was
        // pointed at can be read. That URL is the assertion in the framing test below: waiting on the
        // uncropped one would settle the record, and change the pane's box, a decode too early.
        value: vi.fn(function (this: HTMLImageElement) {
            requested.push(this.src);

            return new Promise<void>((resolve, reject) =>
                pending.push({ resolve, reject: () => reject(new Error("undecodable")) }),
            );
        }),
    });

    /** Finishes every decode asked for so far, the way the webview would once the bytes had arrived. */
    const finish = async (how: "resolve" | "reject" = "resolve") => {
        const waiting = pending.splice(0);

        await act(async () => {
            for (const decode of waiting) decode[how]();
        });
    };

    return { pending, requested, finish };
};

const choose = (index: number) => act(() => useFileStore.getState().setCurrentIndex(index));

beforeEach(() => {
    resetFileStore();
    resetCropStore();
});

afterEach(() => {
    delete (HTMLImageElement.prototype as { decode?: unknown }).decode;
});

describe("the settled file", () => {
    it("is the current one at once, so the first photograph is not delayed by a decode", () => {
        controllable();
        openFiles(HOLIDAY, SUNSET);

        // Nothing is on screen when the canvas mounts, so there is no photograph for the swap to be
        // atomic with and nothing to gain by waiting.
        expect(renderHook(() => useSettledFile()).result.current).toBe(HOLIDAY);
    });

    it("holds the outgoing photograph until the incoming one can be painted", async () => {
        const decode = controllable();
        openFiles(HOLIDAY, SUNSET);

        const { result } = renderHook(() => useSettledFile());
        choose(1);

        // The whole of the bug: the canvas fits the `<img>` to the record's own dimensions, so a
        // record that arrived before its pixels drew 3000x2000 of holiday inside a 6000x4000 box.
        expect(result.current).toBe(HOLIDAY);
        expect(useFileStore.getState().files.at(useFileStore.getState().currentIndex)).toBe(SUNSET);

        await decode.finish();

        expect(result.current).toBe(SUNSET);
    });

    it("swaps to a photograph whose pixels the protocol refuses rather than holding the last one", async () => {
        const decode = controllable();
        openFiles(HOLIDAY, SUNSET);

        const { result } = renderHook(() => useSettledFile());
        choose(1);
        await decode.finish("reject");

        // A rendition that cannot be served still has to reach the canvas, or the window would be
        // showing one photograph under another one's name for the rest of the session.
        expect(result.current).toBe(SUNSET);
    });

    it("waits for nothing on a file whose bytes could not be read", () => {
        const decode = controllable();
        // Built by leaving the key out rather than setting it to `undefined`, which under
        // `exactOptionalPropertyTypes` is not the same record.
        const { identity, ...unreadable } = SUNSET;
        openFiles(HOLIDAY, unreadable);

        const { result } = renderHook(() => useSettledFile());
        choose(1);

        // There is no rendition to preload, so there is nothing a wait could be waiting for.
        expect(decode.pending).toHaveLength(0);
        expect(result.current).toBe(unreadable);
    });

    it("waits on the framed rendition rather than on the photograph", async () => {
        const decode = controllable();
        openFiles(HOLIDAY, SUNSET);
        frame(SUNSET);

        const { result } = renderHook(() => useSettledFile());
        choose(1);

        // The canvas sizes its box from the crop's own rectangle, so the framing and the pixels have
        // to arrive on the same render for the same reason a change of image does. Waiting on the
        // uncropped URL would swap the box a decode before the framed pixels existed - which is the
        // distortion this hook was written to remove, reintroduced by a crop.
        expect(decode.requested.at(-1)).toBe(`opai://localhost/${SUNSET.identity}?crop=${cropQuery(FRAMING)}`);

        await decode.finish();

        expect(result.current).toBe(SUNSET);
    });

    it("settles on the last image chosen, not on whichever decode finishes", async () => {
        const decode = controllable();
        openFiles(HOLIDAY, SUNSET);

        const { result } = renderHook(() => useSettledFile());
        choose(1);
        choose(0);

        // The first choice's decode is abandoned as the second replaces it: a click during a slow
        // decode would otherwise land on the canvas after the click that overtook it.
        await decode.finish();

        expect(result.current).toBe(HOLIDAY);
    });
});
