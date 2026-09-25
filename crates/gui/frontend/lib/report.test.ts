import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { sendError } from "@/lib/faro";
import { alreadyRecorded, forget, report, reportCrash } from "./report";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/lib/faro", () => ({ sendError: vi.fn() }));

const invoked = invoke as unknown as Mock;
const faroed = sendError as unknown as Mock;

/** The records sent so far, once the sends queued behind a microtask have run. */
const sentRecords = async () => {
    await Promise.resolve();
    await Promise.resolve();

    return invoked.mock.calls.map(([command, args]) => {
        expect(command).toBe("log");
        return args.record;
    });
};

let consoleError: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
    forget();
    faroed.mockReset();
    invoked.mockResolvedValue(undefined);
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
});

describe("report", () => {
    it("writes to the console, as every call site did", () => {
        const error = new Error("boom");

        report("drawing failed", error);

        expect(consoleError).toHaveBeenCalledWith("drawing failed", error);
    });

    it("does not send a command's tagged rejection, which Rust has already recorded", async () => {
        report("enhancing the image failed", { kind: "enhance", message: "the model refused" });

        expect(consoleError).toHaveBeenCalledOnce();
        expect(await sentRecords()).toEqual([]);
        expect(faroed).not.toHaveBeenCalled();
    });

    it("sends an Error with its name, message and stack", async () => {
        const error = new TypeError("x is undefined");

        report("drawing failed", error);

        expect(await sentRecords()).toEqual([
            { level: "warn", message: "drawing failed", error: "TypeError: x is undefined", stack: error.stack },
        ]);
    });

    it("sends an Error even when it carries a kind", async () => {
        const error = Object.assign(new Error("tagged"), { kind: "enhance" });

        expect(alreadyRecorded(error)).toBe(false);
        report("m", error);

        expect(await sentRecords()).toHaveLength(1);
    });

    it("sends a string as the error itself", async () => {
        report("the update check could not be asked", "command is_outdated not found");

        expect(await sentRecords()).toEqual([
            {
                level: "warn",
                message: "the update check could not be asked",
                error: "command is_outdated not found",
            },
        ]);
    });

    it("sends a plain object as JSON, and anything else as its string", async () => {
        report("plain", { reason: "denied" });
        report("number", 42);

        const [plain, number] = await sentRecords();
        expect(plain.error).toBe('{"reason":"denied"}');
        expect(number.error).toBe("42");
    });

    it("sends a repeat within one load once, to the log and to Faro", async () => {
        for (let i = 0; i < 5; i += 1) report("drawing failed", new Error("boom"));

        expect(await sentRecords()).toHaveLength(1);
        expect(faroed).toHaveBeenCalledOnce();
        expect(consoleError).toHaveBeenCalledTimes(5);
    });

    it("sends one drop notice for the 51st distinct record, and nothing for the 52nd", async () => {
        for (let i = 1; i <= 52; i += 1) report(`failure ${i}`, "e");

        const records = await sentRecords();
        expect(records).toHaveLength(51);
        expect(records[49].message).toBe("failure 50");
        expect(records[50]).toEqual({
            level: "warn",
            message: "the window is recording too many failures; the rest of this load's are dropped",
        });

        // Faro gets the fifty failures, and not the notice, which is about the file.
        expect(faroed).toHaveBeenCalledTimes(50);
        expect(faroed.mock.lastCall?.[1]).toEqual({ type: "warn", message: "failure 50" });
    });

    it("sends an Error to Faro as itself, with the warning's context", () => {
        const error = new TypeError("x is undefined");

        report("drawing failed", error);

        expect(faroed).toHaveBeenCalledExactlyOnceWith(error, { type: "warn", message: "drawing failed" });
    });

    it("sends anything else to Faro as an Error carrying the record's text", async () => {
        report("plain", { reason: "denied" });
        report("string", "command is_outdated not found");

        const [plain, string] = await sentRecords();
        const [plainError, stringError] = faroed.mock.calls.map(([error]) => error);
        expect(plainError).toBeInstanceOf(Error);
        expect(plainError.message).toBe(plain.error);
        expect(stringError.message).toBe(string.error);
    });

    it("still sends the log record when Faro throws", async () => {
        faroed.mockImplementation(() => {
            throw new Error("faro broke");
        });

        expect(() => report("m", "e")).not.toThrow();
        expect(await sentRecords()).toHaveLength(1);
    });

    it("writes nothing more and throws nothing when the send rejects", async () => {
        invoked.mockRejectedValue(new Error("the bridge is gone"));

        expect(() => report("m", "e")).not.toThrow();
        await sentRecords();
        await new Promise((resolve) => setTimeout(resolve));

        expect(consoleError).toHaveBeenCalledOnce();
        expect(invoked).toHaveBeenCalledOnce();
    });

    it("throws nothing when invoke throws rather than rejects", async () => {
        invoked.mockImplementation(() => {
            throw new TypeError("no Tauri here");
        });

        expect(() => report("m", "e")).not.toThrow();
        await sentRecords();
        await new Promise((resolve) => setTimeout(resolve));
    });
});

describe("reportCrash", () => {
    it("writes to the console and sends at error severity with the component stack", async () => {
        const error = new Error("render");

        reportCrash(error, "\n    at Canvas\n    at App");

        expect(consoleError).toHaveBeenCalledOnce();
        expect(await sentRecords()).toEqual([
            {
                level: "error",
                message: "the window could not be drawn",
                error: "Error: render",
                stack: error.stack,
                componentStack: "\n    at Canvas\n    at App",
            },
        ]);
        expect(faroed).toHaveBeenCalledExactlyOnceWith(error, {
            type: "crash",
            componentStack: "\n    at Canvas\n    at App",
        });
    });

    it("sends a crash with no component stack to Faro without one", () => {
        const error = new Error("render");

        reportCrash(error);

        expect(faroed).toHaveBeenCalledExactlyOnceWith(error, { type: "crash" });
    });
});
