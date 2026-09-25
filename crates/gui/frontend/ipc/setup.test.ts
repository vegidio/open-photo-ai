import { Channel, invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { initialize, quit, type SetupError, type SetupEvent, type SupportedProviders } from "./setup";

// Mocked at the `invoke` boundary, so the names asserted below are the ones that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn
// initialize` and `fn quit` in crates/gui/src/setup.rs; renaming either side alone is what this
// exists to catch.
//
// `Channel` itself is the *real* class - only `invoke` is replaced - so what the wrapper hands over
// is what Tauri would hand over, and a report driven below travels the real ordering machinery
// rather than a stub of it.
vi.mock("@tauri-apps/api/core", async (original) => ({
    ...(await original<typeof import("@tauri-apps/api/core")>()),
    invoke: vi.fn(),
}));

const invoked = invoke as unknown as Mock;

/**
 * The webview global `new Channel()` reads, which jsdom does not have.
 *
 * Stubbed rather than mocked away because the constructor's registration is the mechanism under test:
 * `deliver` below is the callback Rust would be given the id of, and calling it is what a `send` from
 * the Rust side amounts to.
 */
let deliver: (raw: { index: number; message: SetupEvent }) => void;

beforeEach(() => {
    // Through `Reflect` rather than an assignment: `@tauri-apps/api` declares no `Window` member for
    // this global - the webview installs it - so there is nothing for an assignment to widen, and
    // declaring one here would put a fiction about the DOM in a test file.
    Reflect.set(window, "__TAURI_INTERNALS__", {
        transformCallback: (callback: (raw: { index: number; message: SetupEvent }) => void) => {
            deliver = callback;
            return 1;
        },
        unregisterCallback: () => {},
    });
});

afterEach(() => {
    // `unstubGlobals` in vite.config.ts only undoes `vi.stubGlobal`; this is a plain property.
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("initialize", () => {
    it("calls the command Rust registers, by name, handing over a channel", () => {
        initialize(() => {});

        expect(invoked).toHaveBeenCalledWith("initialize", { channel: expect.any(Channel) });
    });

    it("has the handler attached before the invoke, so the plan cannot be missed", () => {
        // The whole reason this is a channel rather than two global events: the plan fires within
        // milliseconds of the command starting, and a listener registered in parallel with the call
        // can miss it. Delivering a message without awaiting anything is what shows the handler was
        // already in place when the channel went over.
        const seen: SetupEvent[] = [];
        initialize((event) => seen.push(event));

        const plan: SetupEvent = { kind: "plan", rows: [{ name: "ONNX Runtime", size: 184 }] };
        deliver({ index: 0, message: plan });

        expect(seen).toEqual([plan]);
    });

    it("passes every event through untouched", () => {
        // No mapping and no defaulting on the way in: the store is the one place a report becomes a
        // row, and normalising here would be a second opinion about what Rust said.
        const seen: SetupEvent[] = [];
        initialize((event) => seen.push(event));

        const plan: SetupEvent = { kind: "plan", rows: [{ name: "NVIDIA CUDA", size: 612 }] };
        const progress: SetupEvent = { kind: "progress", name: "NVIDIA CUDA", state: "extracting", fraction: 0.9 };
        deliver({ index: 0, message: plan });
        deliver({ index: 1, message: progress });

        expect(seen).toEqual([plan, progress]);
    });

    it("resolves with the provider report Rust answered, untouched", async () => {
        // The half of the contract Rust's `the_report_serializes_as_the_fields_a_front_end_reads` pins
        // from the other side: the four keys the processor list is built from, spelled as `serde`
        // emits them. Nothing maps or defaults them on the way in, for the same reason nothing maps
        // the events.
        const report: SupportedProviders = { cpu: true, coreml: true, cuda: false, tensorrt: false, webgpu: false };
        invoked.mockResolvedValueOnce(report);

        await expect(initialize(() => {})).resolves.toEqual(report);
    });

    it("carries a provider it does not know about rather than dropping it", async () => {
        // `SupportedProviders` is `#[non_exhaustive]` in Rust, so a provider added later is a fifth
        // key. The type names the four it knows and the value arrives whole: an unknown provider is
        // one nothing offers, which is correct until someone teaches this side the name, and is the
        // opposite of a launch that fails over a field nobody asked for.
        const widened = { cpu: true, coreml: false, cuda: true, tensorrt: true, rocm: true };
        invoked.mockResolvedValueOnce(widened);

        await expect(initialize(() => {})).resolves.toEqual(widened);
    });

    it("rejects with the whole of what Rust serialized, untouched", async () => {
        // The shape `SetupError` is written against, spelled the way `serde` emits it: `kind` tags
        // the variant and `failure` is the classification beside it. This is the half of the
        // contract that Rust's `the_error_crossing_ipc_carries_the_librarys_own_sentence_and_its_kind`
        // pins from the other side - the two assert the same JSON.
        const rejection: SetupError = {
            kind: "initialize",
            failure: "transfer",
            message: "downloading https://example.invalid/runtime.7z failed: connection reset",
        };
        invoked.mockRejectedValueOnce(rejection);

        // Nothing here maps, wraps or defaults the rejection, for the same reason nothing maps the
        // events: the store is where a failure becomes something the dialog draws.
        await expect(initialize(() => {})).rejects.toEqual(rejection);
    });
});

describe("quit", () => {
    it("calls the command Rust registers, by name", () => {
        quit();

        expect(invoked).toHaveBeenCalledWith("quit");
    });
});
