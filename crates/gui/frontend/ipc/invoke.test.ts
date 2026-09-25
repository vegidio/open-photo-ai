import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { traced } from "@/lib/faro";
import { call } from "./invoke";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => "answered") }));
// Untraced unless a test says otherwise, as before Faro starts.
vi.mock("@/lib/faro", () => ({ traced: vi.fn((_name: string, send: () => Promise<unknown>) => send()) }));

const invoked = invoke as unknown as Mock;
const tracing = traced as unknown as Mock;

const TRACEPARENT = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

/** Makes the next request traced, handing `send` the header a started Faro would. */
const traceNext = () =>
    tracing.mockImplementationOnce((_name: string, send: (headers?: Record<string, string>) => Promise<unknown>) =>
        send({ traceparent: TRACEPARENT }),
    );

describe("call", () => {
    it("names the span after the command, and answers what Rust answered", async () => {
        await expect(call("enhance", { imageId: 1 })).resolves.toBe("answered");

        expect(tracing).toHaveBeenCalledWith("enhance", expect.any(Function));
    });

    it("invokes exactly as before when there is no header", async () => {
        await call("enhance", { imageId: 1 });
        await call("version");

        expect(invoked.mock.calls).toEqual([["enhance", { imageId: 1 }], ["version"]]);
    });

    it("sends the traceparent in the headers, beside the arguments", async () => {
        traceNext();
        await call("enhance", { imageId: 1 });
        traceNext();
        await call("version");

        expect(invoked.mock.calls).toEqual([
            ["enhance", { imageId: 1 }, { headers: { traceparent: TRACEPARENT } }],
            ["version", undefined, { headers: { traceparent: TRACEPARENT } }],
        ]);
    });
});
