import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { cancelSuggest, type SuggestError, type Suggestion, suggest } from "./autopilot";
import type { CropInfo } from "./crop";

// Mocked at the `invoke` boundary, so the names asserted below are the ones that would actually go on the
// wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn suggest` and
// `fn cancel_suggest` in crates/gui/src/autopilot.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

/** One framing, as the Crop/Rotate dialog hands it over. */
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
});

describe("suggest", () => {
    it("calls the command Rust registers, by name, with the arguments its signature takes", () => {
        const { run } = suggest("0123456789abcdef", "coreml", ["upscale", "colorization"]);

        expect(invoked).toHaveBeenCalledWith("suggest", {
            run,
            source: "0123456789abcdef",
            processor: "coreml",
            families: ["upscale", "colorization"],
            crop: undefined,
        });
    });

    it("sends the framing the photograph is to be analysed at", () => {
        const { run } = suggest("0123456789abcdef", "auto", ["face_recovery"], framing);

        expect(invoked).toHaveBeenCalledWith("suggest", {
            run,
            source: "0123456789abcdef",
            processor: "auto",
            families: ["face_recovery"],
            crop: framing,
        });
    });

    it("sends an empty set of families as it is, rather than widening it", () => {
        suggest("0123456789abcdef", "cpu", []);

        expect(invoked).toHaveBeenCalledWith("suggest", expect.objectContaining({ families: [] }));
    });

    it("hands back the run's name in the same turn, before the command has settled", () => {
        let dispatched = false;
        invoked.mockImplementation(() => {
            dispatched = true;
            return new Promise(() => {});
        });

        const { run } = suggest("0123456789abcdef", "cpu", ["upscale"]);

        expect(run).toBeTruthy();
        expect(dispatched).toBe(true);
    });

    it("names every analysis differently, so a stop never lands on the wrong one", () => {
        const first = suggest("0123456789abcdef", "cpu", ["upscale"]).run;
        const second = suggest("0123456789abcdef", "cpu", ["upscale"]).run;

        expect(first).not.toBe(second);
    });

    it("answers the suggestions the analysis made", async () => {
        const suggested: Suggestion[] = [{ family: "colorization" }, { family: "upscale", scale: 4 }];
        invoked.mockResolvedValue(suggested);

        await expect(suggest("0123456789abcdef", "cpu", ["upscale", "colorization"]).done).resolves.toEqual(suggested);
    });

    it("rejects with the refusal rather than resolving empty", async () => {
        const refused: SuggestError = { kind: "stopped" };
        invoked.mockRejectedValue(refused);

        await expect(suggest("0123456789abcdef", "cpu", ["upscale"]).done).rejects.toEqual(refused);
    });
});

describe("cancelSuggest", () => {
    it("calls the command Rust registers, by name, naming the run", () => {
        cancelSuggest("session-7");

        expect(invoked).toHaveBeenCalledWith("cancel_suggest", { run: "session-7" });
    });

    it("names the analysis it was given", () => {
        const { run } = suggest("0123456789abcdef", "auto", ["upscale"]);
        cancelSuggest(run);

        expect(invoked).toHaveBeenLastCalledWith("cancel_suggest", { run });
    });
});
