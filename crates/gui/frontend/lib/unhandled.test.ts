import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { forget } from "./report";
import { watchUnhandled } from "./unhandled";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

/** The records sent, once the sends queued behind a microtask have run. */
const sentRecords = async () => {
    await Promise.resolve();
    await Promise.resolve();

    return invoked.mock.calls.filter(([command]) => command === "log").map(([, args]) => args.record);
};

/** jsdom has no `PromiseRejectionEvent`, so the event is built the way the handler reads it. */
const rejection = (reason: unknown) => Object.assign(new Event("unhandledrejection"), { reason });

let unwatch: () => void;

beforeEach(() => {
    forget();
    invoked.mockResolvedValue(undefined);
    vi.spyOn(console, "error").mockImplementation(() => {});
    unwatch = watchUnhandled();
});

afterEach(() => {
    unwatch();
});

describe("watchUnhandled", () => {
    it("sends one warn for a rejection nothing handled", async () => {
        const reason = new Error("probe");

        window.dispatchEvent(rejection(reason));

        expect(await sentRecords()).toEqual([
            {
                level: "warn",
                message: "a promise rejection nothing handled",
                error: "Error: probe",
                stack: reason.stack,
            },
        ]);
    });

    it("sends nothing for a command's tagged rejection nothing handled: Rust has it", async () => {
        window.dispatchEvent(rejection({ kind: "enhance", message: "the model refused" }));

        expect(await sentRecords()).toEqual([]);
    });

    it("sends one warn for an error nothing caught", async () => {
        const error = new RangeError("out of bounds");

        window.dispatchEvent(new ErrorEvent("error", { error, message: "Uncaught RangeError: out of bounds" }));

        const [record] = await sentRecords();
        expect(record).toMatchObject({
            level: "warn",
            message: "an error nothing caught",
            error: "RangeError: out of bounds",
        });
    });

    it("falls back on the event's message where it carries no error", async () => {
        window.dispatchEvent(new ErrorEvent("error", { message: "Script error." }));

        const [record] = await sentRecords();
        expect(record.error).toBe("Script error.");
    });

    it("sends nothing once removed", async () => {
        unwatch();

        window.dispatchEvent(rejection(new Error("late")));

        expect(await sentRecords()).toEqual([]);
    });
});
