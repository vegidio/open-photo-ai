import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import type { CropInfo } from "./crop";
import { type DetectError, detectFaces, type Face } from "./faces";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on the
// wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn detect_faces` in
// crates/gui/src/faces.rs.
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

/** One face, in the shape Rust's `Face` serializes into. */
const face = (left: number): Face => ({
    bounding_box: { min: { x: left, y: 4 }, max: { x: left + 3, y: 7 } },
    landmarks: [
        { x: left + 1, y: 5 },
        { x: left + 2, y: 5 },
        { x: left + 1.5, y: 6 },
        { x: left + 1, y: 6.5 },
        { x: left + 2, y: 6.5 },
    ],
    confidence: 0.9,
    restorable: true,
    key: `${left},4,${left + 3},7`,
});

beforeEach(() => {
    invoked.mockReset();
});

describe("detectFaces", () => {
    it("calls the command Rust registers, by name, with the arguments its signature takes", () => {
        const { run } = detectFaces("0123456789abcdef", "coreml");

        expect(invoked).toHaveBeenCalledWith("detect_faces", {
            run,
            source: "0123456789abcdef",
            processor: "coreml",
            crop: undefined,
        });
    });

    it("sends the framing the faces are to be found in", () => {
        const { run } = detectFaces("0123456789abcdef", "auto", framing);

        expect(invoked).toHaveBeenCalledWith("detect_faces", {
            run,
            source: "0123456789abcdef",
            processor: "auto",
            crop: framing,
        });
    });

    it("hands back the run's name in the same turn, before the command has settled", () => {
        // What makes the name usable by a cleanup that runs before the promise does - the reason this is
        // not simply an `async` function.
        let dispatched = false;
        invoked.mockImplementation(() => {
            dispatched = true;
            return new Promise(() => {});
        });

        const { run } = detectFaces("0123456789abcdef", "cpu");

        expect(run).toBeTruthy();
        expect(dispatched).toBe(true);
    });

    it("names every detection differently, so one window's reports never cross", () => {
        const first = detectFaces("0123456789abcdef", "cpu").run;
        const second = detectFaces("0123456789abcdef", "cpu").run;

        expect(first).not.toBe(second);
    });

    it("answers the faces the detector found, in the order it found them", async () => {
        const found = [face(0), face(20), face(40)];
        invoked.mockResolvedValue(found);

        await expect(detectFaces("0123456789abcdef", "cpu").done).resolves.toEqual(found);
    });

    it("answers no faces for a photograph with nobody in it, which is a finding rather than a failure", async () => {
        invoked.mockResolvedValue([]);

        await expect(detectFaces("0123456789abcdef", "cpu").done).resolves.toEqual([]);
    });

    it("rejects with the refusal rather than resolving empty", async () => {
        const refused: DetectError = { kind: "unknownSource", identity: "0123456789abcdef" };
        invoked.mockRejectedValue(refused);

        await expect(detectFaces("0123456789abcdef", "cpu").done).rejects.toEqual(refused);
    });
});
