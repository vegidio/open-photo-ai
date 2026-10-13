import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { type ExportName, type ExportNaming, exportNames } from "@/ipc/export";
import { useExportNames } from "./useExportNames";

vi.mock("@/ipc/export", () => ({ exportNames: vi.fn() }));

const asked = exportNames as unknown as Mock;

/** The answers the test settles by hand, in the order they were asked for. */
const pending: ((names: ExportName[]) => void)[] = [];

const named = (path: string, name: string): ExportName => ({
    path,
    name,
    destination: `/photos/${name}`,
    format: "png",
    extension: "png",
});

const QUEUE = ["/photos/beach.jpg"];
const plain: ExportNaming = { prefix: "", suffix: "", format: "png" };

beforeEach(() => {
    pending.length = 0;
    asked.mockReset();
    asked.mockImplementation(() => new Promise<ExportName[]>((resolve) => pending.push(resolve)));
});

describe("useExportNames", () => {
    it("is nothing until the first answer, then the names by path", async () => {
        const { result } = renderHook(() => useExportNames(QUEUE, plain));
        expect(result.current).toBeUndefined();
        expect(asked).toHaveBeenCalledExactlyOnceWith(QUEUE, plain);

        await act(async () => pending[0]?.([named("/photos/beach.jpg", "beach.png")]));

        expect(result.current?.byPath.get("/photos/beach.jpg")?.name).toBe("beach.png");
    });

    it("asks again on every change to the naming, keeping the last names until the next answer", async () => {
        const { result, rerender } = renderHook(({ naming }) => useExportNames(QUEUE, naming), {
            initialProps: { naming: plain },
        });
        await act(async () => pending[0]?.([named("/photos/beach.jpg", "beach.png")]));

        rerender({ naming: { ...plain, suffix: "-opai" } });

        expect(asked).toHaveBeenLastCalledWith(QUEUE, { ...plain, suffix: "-opai" });
        expect(result.current?.byPath.get("/photos/beach.jpg")?.name).toBe("beach.png");
    });

    it("drops an answer that arrives after a later one was asked for", async () => {
        const { result, rerender } = renderHook(({ naming }) => useExportNames(QUEUE, naming), {
            initialProps: { naming: { ...plain, suffix: "-o" } },
        });
        rerender({ naming: { ...plain, suffix: "-op" } });

        // The later answer first, then the earlier one, as fast typing can deliver them.
        await act(async () => pending[1]?.([named("/photos/beach.jpg", "beach-op.png")]));
        await act(async () => pending[0]?.([named("/photos/beach.jpg", "beach-o.png")]));

        expect(result.current?.byPath.get("/photos/beach.jpg")?.name).toBe("beach-op.png");
    });

    it("says which format choice the names were answered for, which lags the settings until the next answer", async () => {
        const { result, rerender } = renderHook(({ naming }) => useExportNames(QUEUE, naming), {
            initialProps: { naming: plain },
        });
        await act(async () => pending[0]?.([named("/photos/beach.jpg", "beach.png")]));
        expect(result.current?.format).toBe("png");

        rerender({ naming: { ...plain, format: "preserve" } });
        expect(result.current?.format).toBe("png");

        await act(async () => pending[1]?.([named("/photos/beach.jpg", "beach.jpg")]));
        expect(result.current?.format).toBe("preserve");
    });

    it("asks again when the queue changes", () => {
        const { rerender } = renderHook(({ queue }) => useExportNames(queue, plain), {
            initialProps: { queue: QUEUE },
        });

        rerender({ queue: [...QUEUE, "/photos/shot.nef"] });

        expect(asked).toHaveBeenLastCalledWith([...QUEUE, "/photos/shot.nef"], plain);
    });
});
