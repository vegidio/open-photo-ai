import { invoke } from "@tauri-apps/api/core";
import { beforeAll, describe, expect, it, type Mock, vi } from "vitest";
import type { WindowRecord } from "@/ipc/log";
import { windowReady } from "@/ipc/window";

// The entry point itself, imported once, rather than a copy of its tree: what is under test is where
// `main.tsx` puts the boundary, the handlers and `RevealWindow` relative to each other.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@/ipc/window", () => ({ windowReady: vi.fn(() => Promise.resolve()) }));
vi.mock("@/lib/analytics", () => ({ mirrorAnalytics: vi.fn() }));
vi.mock("./App", () => ({
    default: () => {
        throw new Error("the first render failed");
    },
}));

const invoked = invoke as unknown as Mock;

// Read once the entry point has settled, and kept here: `clearMocks` empties every spy before each test.
let shown = false;
let records: WindowRecord[] = [];

describe("main.tsx, with an App that throws on its first render", () => {
    beforeAll(async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        document.body.innerHTML = '<div id="root"></div>';

        await import("./main");

        await vi.waitFor(() => expect(windowReady).toHaveBeenCalled());
        // Past the sends queued behind a microtask, and past any `error` event React might dispatch.
        await new Promise((resolve) => setTimeout(resolve, 20));

        shown = vi.mocked(windowReady).mock.calls.length > 0;
        records = invoked.mock.calls.filter(([command]) => command === "log").map(([, args]) => args.record);
    });

    it("still shows the window, with the recovery screen on it", () => {
        expect(shown).toBe(true);
        expect(document.querySelector("[role='alert']")).toHaveTextContent("Error: the first render failed");
    });

    it("sends exactly one record of the crash, with the handlers installed", () => {
        const [record, ...more] = records;
        expect(more).toEqual([]);
        expect(record).toMatchObject({ level: "error", error: "Error: the first render failed" });
        expect(record?.componentStack).toMatch(/\S/);
    });
});
