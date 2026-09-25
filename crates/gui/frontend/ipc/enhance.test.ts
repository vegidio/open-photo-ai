import { invoke } from "@tauri-apps/api/core";
import { type Event, listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import type { CropInfo } from "./crop";
import {
    cancelEnhance,
    type EnhanceError,
    type Enhancement,
    enhance,
    type Operation,
    onEnhanceProgress,
    type RunProgress,
    releaseAllEnhanced,
    releaseEnhanced,
} from "./enhance";

// Mocked at the `invoke` and `listen` boundaries, so the names asserted below are the ones that would
// actually go on the wire. This is the frontend half of a contract whose Rust half is
// `#[tauri::command] fn enhance`, `fn cancel_enhance`, `fn release_enhanced` and `fn
// release_all_enhanced` in crates/gui/src/enhance/mod.rs, plus the `PROGRESS_EVENT` constant beside
// them; renaming either side alone is what this exists to catch.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const invoked = invoke as unknown as Mock;
const listened = listen as unknown as Mock;

/** One upscale, as a chooser built from the catalogue would hand it over. */
const kyoto: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 };

/**
 * One framing, as the Crop/Rotate dialog will hand it over.
 *
 * Injected, because nothing in this application sets a crop until that dialog lands. The field names
 * are the contract: Rust's `Wire` in crates/gui/src/images/crop.rs deserializes exactly these, with
 * `deny_unknown_fields`, so a rename on either side alone is a command that rejects at runtime.
 */
const framing: CropInfo = {
    left: 10,
    top: 20,
    width: 30,
    height: 40,
    millidegrees: -1500,
    flipHorizontal: true,
    flipVertical: false,
};

beforeEach(() => {
    invoked.mockReset();
    listened.mockReset();
});

describe("enhance", () => {
    it("calls the command Rust registers, by name, with what the run takes", () => {
        const { run } = enhance("0123456789abcdef", [kyoto], "coreml");

        expect(invoked).toHaveBeenCalledWith("enhance", {
            run,
            source: "0123456789abcdef",
            operations: [kyoto],
            processor: "coreml",
            crop: undefined,
        });
    });

    it("sends the framing under the name and shape Rust deserializes", () => {
        // The argument's name and every field of it, pinned here because nothing in either toolchain
        // notices when one side is renamed alone. Rust's half is `crop: Option<Crop>` on the command
        // and the `Wire` struct behind it, which refuses a field it does not know.
        const { run } = enhance("0123456789abcdef", [kyoto], "auto", framing);

        expect(invoked).toHaveBeenCalledWith("enhance", {
            run,
            source: "0123456789abcdef",
            operations: [kyoto],
            processor: "auto",
            crop: {
                left: 10,
                top: 20,
                width: 30,
                height: 40,
                millidegrees: -1500,
                flipHorizontal: true,
                flipVertical: false,
            },
        });
    });

    it("sends no framing when it is given none", () => {
        // A run over the whole photograph, which is every run this application makes until the dialog
        // ships. Rust reads the absent argument as `None` and runs over the file's own pixels.
        enhance("0123456789abcdef", [kyoto], "auto");

        const [, args] = invoked.mock.calls[0] as [string, { crop?: CropInfo }];

        expect(args.crop).toBeUndefined();
    });

    it("mints the run's name before the command is called", () => {
        // The property the whole cancellation story rests on: a stop and its run cross the boundary
        // independently and either may arrive first, so an effect that starts a run must have the
        // name of the thing its cleanup will stop in the same turn - not once the promise settles.
        let nameAtCall: unknown;
        invoked.mockImplementation((_command: string, args: { run: string }) => {
            nameAtCall = args.run;

            return new Promise(() => {});
        });

        const { run } = enhance("0123456789abcdef", [kyoto], "auto");

        expect(run).toEqual(expect.any(String));
        expect(nameAtCall).toBe(run);
    });

    it("gives each run a name of its own", () => {
        // Two runs sharing a name would mean a stop for the first landing on the second, which is the
        // one thing naming the run was meant to prevent.
        const first = enhance("0123456789abcdef", [kyoto], "auto").run;
        const second = enhance("0123456789abcdef", [kyoto], "auto").run;

        expect(first).not.toBe(second);
    });

    it("resolves with the outcome Rust answered, untouched", async () => {
        const enhanced: Enhancement = {
            outcome: "enhanced",
            identity: "fedcba9876543210",
            width: 6000,
            height: 4000,
        };
        invoked.mockResolvedValueOnce(enhanced);

        await expect(enhance("0123456789abcdef", [kyoto], "auto").done).resolves.toEqual(enhanced);
    });

    it("treats a stopped run as an outcome rather than a failure", async () => {
        // A user who has changed their mind has not been told their enhancement broke, so this
        // resolves rather than rejecting and nothing here turns it back into an error.
        invoked.mockResolvedValueOnce({ outcome: "stopped" } satisfies Enhancement);

        await expect(enhance("0123456789abcdef", [kyoto], "auto").done).resolves.toEqual({ outcome: "stopped" });
    });

    it("accepts an empty chain, which is a request rather than a mistake", () => {
        const { run } = enhance("0123456789abcdef", [], "cpu");

        expect(invoked).toHaveBeenCalledWith("enhance", {
            run,
            source: "0123456789abcdef",
            operations: [],
            processor: "cpu",
        });
    });

    it("propagates a refusal as Rust serialized it", async () => {
        const refused: EnhanceError = { kind: "unknownSource", identity: "0123456789abcdef" };
        invoked.mockRejectedValueOnce(refused);

        await expect(enhance("0123456789abcdef", [kyoto], "auto").done).rejects.toEqual(refused);
    });
});

describe("cancelEnhance", () => {
    it("calls the command Rust registers, by name, naming the run", () => {
        cancelEnhance("session-7");

        expect(invoked).toHaveBeenCalledWith("cancel_enhance", { run: "session-7" });
    });

    it("names the run it was given rather than whatever is running", () => {
        // `cancel_enhance` takes a name because "stop whatever is running" would kill a successor the
        // window had already started - the race design.md D3 is written for.
        const { run } = enhance("0123456789abcdef", [kyoto], "auto");
        cancelEnhance(run);

        expect(invoked).toHaveBeenLastCalledWith("cancel_enhance", { run });
    });
});

describe("releaseEnhanced", () => {
    it("calls the command Rust registers, by name, naming the image", () => {
        releaseEnhanced("0123456789abcdef");

        expect(invoked).toHaveBeenCalledWith("release_enhanced", { identity: "0123456789abcdef" });
    });

    it("names the photograph that was closed rather than the result it produced", () => {
        // The identity on the record the file store is removing. Naming the result instead would need a
        // lookup this side does not have, and naming nothing at all would drop a result belonging to a
        // photograph that is still open - see the Rust command's own documentation.
        const closed = "0123456789abcdef";
        const produced = "fedcba9876543210";

        releaseEnhanced(closed);

        expect(invoked).toHaveBeenLastCalledWith("release_enhanced", { identity: closed });
        expect(invoked).not.toHaveBeenCalledWith("release_enhanced", { identity: produced });
    });

    it("propagates a rejection rather than swallowing it here", async () => {
        // Swallowed at the call site, which is where the decision not to surface it belongs - this
        // wrapper is the wire and nothing more.
        invoked.mockRejectedValueOnce(new Error("the window is gone"));

        await expect(releaseEnhanced("0123456789abcdef")).rejects.toThrow("the window is gone");
    });
});

describe("releaseAllEnhanced", () => {
    it("calls the command Rust registers, by name, naming nothing", () => {
        releaseAllEnhanced();

        expect(invoked).toHaveBeenCalledWith("release_all_enhanced");
    });
});

/**
 * The callback `listen` was registered with, which is what a `Emitter::emit` from Rust amounts to.
 *
 * Read back through the mock rather than held in a variable, and asserted rather than cast: the
 * subscription happening at all is half of what these tests are about.
 */
const delivering = () => {
    const registered = listened.mock.calls[0]?.[1];
    if (typeof registered !== "function") throw new Error("nothing subscribed to the progress event");

    return registered as (event: Event<RunProgress>) => void;
};

describe("onEnhanceProgress", () => {
    it("subscribes to the event Rust emits under", () => {
        onEnhanceProgress(() => {});

        expect(listened).toHaveBeenCalledWith("enhance:progress", expect.any(Function));
    });

    it("hands the handler the report rather than the event carrying it", () => {
        const seen: RunProgress[] = [];
        onEnhanceProgress((report) => seen.push(report));

        const report: RunProgress = {
            run: "session-1",
            operation: "Kyoto 4x (FP32)",
            family: "upscale",
            stage: "installing",
            chainFraction: 0.02,
            installFraction: 0.41,
        };
        delivering()({ event: "enhance:progress", id: 1, payload: report });

        expect(seen).toEqual([report]);
    });

    it("carries an operation that was already known with neither stage nor fetch", () => {
        // The two absences are the whole of how a cached operation is reported: it did neither of the
        // things a label can name, so the interface draws no indicator and the bar simply advances.
        const seen: RunProgress[] = [];
        onEnhanceProgress((report) => seen.push(report));

        delivering()({
            event: "enhance:progress",
            id: 1,
            payload: { run: "session-1", operation: "Kyoto 4x (FP32)", family: "upscale", chainFraction: 1 },
        });

        const [report] = seen;
        expect(report?.stage).toBeUndefined();
        expect(report?.installFraction).toBeUndefined();
        expect(report?.chainFraction).toBe(1);

        // And it still names the enhancement it belongs to: the operation did no work, but a chain
        // made entirely of such operations produces this and nothing else.
        expect(report?.family).toBe("upscale");
    });

    it("carries the enhancement's family beside the composed operation name", () => {
        // The field's spelling, pinned on this side. `enhance/progress.rs` pins the other half against the
        // catalogue's own `Family`, and nothing in either toolchain notices when one is renamed
        // alone: the window would simply stop finding the enhancement and fall back to the generic
        // label.
        const seen: RunProgress[] = [];
        onEnhanceProgress((report) => seen.push(report));

        delivering()({
            event: "enhance:progress",
            id: 1,
            payload: { run: "session-1", operation: "Kyoto 4x (FP32)", family: "upscale", chainFraction: 0.5 },
        });

        expect(seen[0]?.family).toBe("upscale");
    });
});
