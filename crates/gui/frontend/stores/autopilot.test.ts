import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { cancelSuggest } from "@/ipc/autopilot";
import { FRAMING, HOLIDAY, openFiles, resetFileStore, SUNSET } from "@/test/support";
import { useAutopilotStore } from "./autopilot";
import { useFileStore } from "./files";

// The stop is the one thing this store asks Rust for; what the seam sends is `ipc/autopilot.test.ts`'s subject.
vi.mock("@/ipc/autopilot", () => ({ cancelSuggest: vi.fn(() => Promise.resolve()) }));

// Closing a file tells every registered owner, and the enhancement store is one that hands a result back.
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

const cancelled = cancelSuggest as unknown as Mock;

const analysing = () => useAutopilotStore.getState().analysing;

beforeEach(() => {
    vi.clearAllMocks();
    useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);
    resetFileStore();
});

describe("the analyses in flight", () => {
    it("records one per photograph, with the framing it was asked at", () => {
        useAutopilotStore.getState().begin(HOLIDAY.path, "run-1", FRAMING);
        useAutopilotStore.getState().begin(SUNSET.path, "run-2", undefined);

        expect(analysing().get(HOLIDAY.path)).toEqual({ run: "run-1", crop: FRAMING });
        expect(analysing().get(SUNSET.path)).toEqual({ run: "run-2" });
    });

    it("ends the entry that names the run", () => {
        const { begin, end } = useAutopilotStore.getState();

        begin(HOLIDAY.path, "run-1", undefined);
        end(HOLIDAY.path, "run-1");

        expect(analysing().has(HOLIDAY.path)).toBe(false);
    });

    it("leaves a newer entry alone when a stale run ends", () => {
        const { begin, end } = useAutopilotStore.getState();

        // A reframe stopped run-1 and asked again as run-2; run-1 settling late must not take run-2's spinner.
        begin(HOLIDAY.path, "run-1", undefined);
        begin(HOLIDAY.path, "run-2", FRAMING);
        end(HOLIDAY.path, "run-1");

        expect(analysing().get(HOLIDAY.path)).toEqual({ run: "run-2", crop: FRAMING });
    });

    it("stops one photograph's analysis by name and forgets it", () => {
        const { begin, stop } = useAutopilotStore.getState();

        begin(HOLIDAY.path, "run-1", undefined);
        begin(SUNSET.path, "run-2", undefined);
        stop(HOLIDAY.path);

        expect(cancelled).toHaveBeenCalledExactlyOnceWith("run-1");
        expect(analysing().has(HOLIDAY.path)).toBe(false);
        expect(analysing().has(SUNSET.path)).toBe(true);
    });

    it("asks nothing to stop a photograph with nothing in flight", () => {
        useAutopilotStore.getState().stop(HOLIDAY.path);

        expect(cancelled).not.toHaveBeenCalled();
    });

    it("stops every analysis by name and forgets them all", () => {
        const { begin, stopAll } = useAutopilotStore.getState();

        begin(HOLIDAY.path, "run-1", undefined);
        begin(SUNSET.path, "run-2", undefined);
        stopAll();

        expect(cancelled.mock.calls).toEqual([["run-1"], ["run-2"]]);
        expect(analysing().size).toBe(0);
    });
});

describe("a declined photograph", () => {
    it("is recorded and cleared", () => {
        const { decline, clearDeclined } = useAutopilotStore.getState();

        decline(HOLIDAY.path);
        expect(useAutopilotStore.getState().declined).toBe(HOLIDAY.path);

        clearDeclined();
        // Absent rather than `undefined`, which is what an optional property means here.
        expect("declined" in useAutopilotStore.getState()).toBe(false);
    });
});

describe("closing a photograph being analysed", () => {
    beforeEach(() => {
        openFiles(HOLIDAY, SUNSET);

        const { begin } = useAutopilotStore.getState();
        begin(HOLIDAY.path, "run-1", undefined);
        begin(SUNSET.path, "run-2", undefined);
    });

    it("stops only that photograph's analysis", () => {
        useFileStore.getState().closeFile(HOLIDAY.path);

        expect(cancelled).toHaveBeenCalledExactlyOnceWith("run-1");
        expect([...analysing().keys()]).toEqual([SUNSET.path]);
    });

    it("stops every analysis when every photograph is closed", () => {
        useFileStore.getState().closeAll();

        expect(cancelled.mock.calls).toEqual([["run-1"], ["run-2"]]);
        expect(analysing().size).toBe(0);
    });
});
