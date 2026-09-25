import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { cancelExport } from "@/ipc/export";
import { useAutopilotStore } from "@/stores/autopilot";
import { batchSummary, overallFraction, useExportBatchStore } from "./exportBatch";

vi.mock("@/ipc/export", () => ({ cancelExport: vi.fn(() => Promise.resolve()) }));
vi.mock("@/ipc/autopilot", () => ({ cancelSuggest: vi.fn(() => Promise.resolve()) }));

const cancelled = cancelExport as unknown as Mock;

const store = () => useExportBatchStore.getState();
const summary = () => batchSummary(store());
const stages = () => store().queue.map((path) => store().rows.get(path)?.stage);

beforeEach(() => {
    vi.clearAllMocks();
    useExportBatchStore.setState(useExportBatchStore.getInitialState(), true);
    useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);
});

describe("open and reset", () => {
    it("open snapshots the queue with every row Queued", () => {
        store().open(["/a.jpg", "/b.jpg"]);

        expect(store().phase).toBe("idle");
        expect(store().queue).toEqual(["/a.jpg", "/b.jpg"]);
        expect(stages()).toEqual(["queued", "queued"]);
    });

    it("reset puts every row back to Queued with nothing written, and starts nothing", () => {
        store().open(["/a.jpg", "/b.jpg"]);
        store().start();
        store().setStage("/a.jpg", { stage: "done", path: "/a_1.png", bytes: 10 });
        store().setStage("/b.jpg", { stage: "failed", reason: "no" });
        store().finish();

        store().reset();

        expect(store().phase).toBe("idle");
        expect(stages()).toEqual(["queued", "queued"]);
        expect(summary()).toMatchObject({ written: 0, failed: 0, remaining: 2 });
        expect(store().aborted).toBe(false);
    });
});

describe("the summary", () => {
    it("counts every row as ready while idle", () => {
        store().open(["/a", "/b", "/c", "/d"]);

        expect(summary()).toEqual({ phase: "idle", total: 4, written: 0, failed: 0, remaining: 4 });
    });

    it("counts what is written and what remains after the row in progress while running", () => {
        store().open(["/a", "/b", "/c", "/d"]);
        store().start();
        store().setStage("/a", { stage: "done", path: "/a.png", bytes: 1 });
        store().setCurrent({ path: "/b", run: "run-2" });
        store().setStage("/b", { stage: "enhancing", fraction: 0.5 });

        expect(summary()).toEqual({ phase: "running", total: 4, written: 1, failed: 0, remaining: 2 });
    });

    it("counts the failed rows apart from the written ones once ended", () => {
        store().open(["/a", "/b", "/c", "/d"]);
        store().start();
        store().setStage("/a", { stage: "done", path: "/a.png", bytes: 1 });
        store().setStage("/b", { stage: "failed", reason: "no" });
        store().setStage("/c", { stage: "done", path: "/c.png", bytes: 1 });
        store().setStage("/d", { stage: "done", path: "/d.png", bytes: 1 });
        store().finish();

        expect(summary()).toEqual({ phase: "ended", total: 4, written: 3, failed: 1, remaining: 0 });
        expect("current" in store()).toBe(false);
    });
});

describe("the overall fraction", () => {
    it("is the finished rows plus the fraction of the one in progress, over the number of rows", () => {
        store().open(["/a", "/b", "/c", "/d"]);
        expect(overallFraction(store())).toBe(0);

        store().start();
        store().setStage("/a", { stage: "done", path: "/a.png", bytes: 1 });
        store().setStage("/b", { stage: "failed", reason: "no" });
        store().setCurrent({ path: "/c", run: "run-3" });
        store().setStage("/c", { stage: "enhancing", fraction: 0.5 });

        expect(overallFraction(store())).toBe(2.5 / 4);

        store().setStage("/c", { stage: "writing" });
        expect(overallFraction(store())).toBe(3 / 4);
    });

    it("is nothing for an empty queue", () => {
        store().open([]);

        expect(overallFraction(store())).toBe(0);
    });
});

describe("abort", () => {
    it("stops the export in flight and marks the batch aborted", () => {
        store().open(["/a"]);
        store().start();
        store().setCurrent({ path: "/a", run: "run-1" });

        store().abort();

        expect(store().aborted).toBe(true);
        expect(cancelled).toHaveBeenCalledExactlyOnceWith("run-1");
    });

    it("stops the Autopilot analysis in flight", () => {
        useAutopilotStore.getState().begin("/a", "suggest-1", undefined);
        store().open(["/a"]);
        store().start();
        store().setCurrent({ path: "/a", analysing: true });

        store().abort();

        expect(useAutopilotStore.getState().analysing.has("/a")).toBe(false);
        expect(cancelled).not.toHaveBeenCalled();
    });

    it("does nothing unless a batch is running", () => {
        store().open(["/a"]);

        store().abort();

        expect(store().aborted).toBe(false);
    });
});
