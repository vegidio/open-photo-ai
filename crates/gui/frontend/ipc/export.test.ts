import { invoke } from "@tauri-apps/api/core";
import { type Event, listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import type { CropInfo } from "./crop";
import { EXPORT_FORMATS } from "@/test/support";
import { mintRun, type Operation } from "./enhance";
import {
    cancelExport,
    type ExportError,
    type Exported,
    type ExportProgress,
    type ExportRequest,
    exportFormats,
    exportImage,
    forgetExportFormats,
    onExportProgress,
    pickDirectory,
    revealExport,
} from "./export";

// Mocked at the `invoke` and `listen` boundaries, so the names asserted below are the ones that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn export` and
// `fn cancel_export` in crates/gui/src/export/mod.rs, `fn pick_directory` in directory.rs, `fn reveal_export` in
// written.rs, and `EXPORT_PROGRESS_EVENT` in crates/gui/src/export/progress.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

// The real counter, observed: which is what lets the order of minting and invoking be asserted rather than inferred.
vi.mock("./enhance", async (actual) => {
    const real = await actual<typeof import("./enhance")>();

    return { ...real, mintRun: vi.fn(real.mintRun) };
});

const invoked = invoke as unknown as Mock;
const listened = listen as unknown as Mock;
const minted = mintRun as unknown as Mock;

/** One upscale, as the window would name it. */
const kyoto: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", parameters: { scale: 2 } };

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

/** Where and how, as the export dialog will ask. */
const request: ExportRequest = {
    destination: "/Users/someone/Pictures/holiday-opai.jpg",
    format: "jpeg",
    quality: 90,
    overwrite: false,
};

beforeEach(() => {
    invoked.mockReset();
    listened.mockReset();
    minted.mockClear();
});

describe("exportImage", () => {
    it("calls the command Rust registers, by name, with the arguments its signature takes", () => {
        const { run } = exportImage("0123456789abcdef", [kyoto], "coreml", request, framing);

        expect(invoked).toHaveBeenCalledWith("export", {
            run,
            source: "0123456789abcdef",
            operations: [kyoto],
            processor: "coreml",
            crop: framing,
            destination: "/Users/someone/Pictures/holiday-opai.jpg",
            format: "jpeg",
            quality: 90,
            overwrite: false,
        });
    });

    it("sends no framing as undefined, and an empty chain as it is", () => {
        const { run } = exportImage("0123456789abcdef", [], "auto", { ...request, format: "heic", overwrite: true });

        expect(invoked).toHaveBeenCalledWith("export", {
            run,
            source: "0123456789abcdef",
            operations: [],
            processor: "auto",
            crop: undefined,
            destination: "/Users/someone/Pictures/holiday-opai.jpg",
            format: "heic",
            quality: 90,
            overwrite: true,
        });
    });

    it("mints the export's name from the shared counter before the command is invoked", () => {
        const { run } = exportImage("0123456789abcdef", [kyoto], "cpu", request);

        expect(minted).toHaveBeenCalledOnce();
        expect(minted.mock.results[0]?.value).toBe(run);
        expect(minted.mock.invocationCallOrder[0]).toBeLessThan(invoked.mock.invocationCallOrder[0] ?? 0);
    });

    it("hands back the export's name in the same turn, before the command has settled", () => {
        let dispatched = false;
        invoked.mockImplementation(() => {
            dispatched = true;
            return new Promise(() => {});
        });

        const { run } = exportImage("0123456789abcdef", [kyoto], "cpu", request);

        expect(run).toBeTruthy();
        expect(dispatched).toBe(true);
    });

    it("names every export differently, so a stop never lands on the wrong one", () => {
        const first = exportImage("0123456789abcdef", [kyoto], "cpu", request).run;
        const second = exportImage("0123456789abcdef", [kyoto], "cpu", request).run;

        expect(first).not.toBe(second);
    });

    it("answers what was written", async () => {
        const written: Exported = { outcome: "exported", path: "/Users/someone/Pictures/holiday-opai_1.jpg", bytes: 3 };
        invoked.mockResolvedValue(written);

        await expect(exportImage("0123456789abcdef", [kyoto], "cpu", request).done).resolves.toEqual(written);
    });

    it("rejects with the refusal", async () => {
        const refused: ExportError = { kind: "write", path: request.destination, message: "Permission denied" };
        invoked.mockRejectedValue(refused);

        await expect(exportImage("0123456789abcdef", [kyoto], "cpu", request).done).rejects.toEqual(refused);
    });
});

describe("exportFormats", () => {
    it("calls the command Rust registers, by name, once however often it is asked", async () => {
        forgetExportFormats();
        invoked.mockResolvedValue(EXPORT_FORMATS);

        await expect(exportFormats()).resolves.toEqual(EXPORT_FORMATS);
        await exportFormats();

        expect(invoked).toHaveBeenCalledExactlyOnceWith("export_formats");
    });
});

describe("exportImage and a format that takes no quality", () => {
    it("sends no quality where the request carries none", () => {
        const { destination, format, overwrite } = request;
        const { run } = exportImage("0123456789abcdef", [], "cpu", { destination, format, overwrite });

        expect(invoked).toHaveBeenCalledWith("export", expect.objectContaining({ run, quality: undefined }));
    });
});

describe("cancelExport", () => {
    it("calls the command Rust registers, by name, naming the export", () => {
        cancelExport("session-7");

        expect(invoked).toHaveBeenCalledWith("cancel_export", { run: "session-7" });
    });

    it("names the export it was given", () => {
        const { run } = exportImage("0123456789abcdef", [kyoto], "auto", request);
        cancelExport(run);

        expect(invoked).toHaveBeenLastCalledWith("cancel_export", { run });
    });
});

/** The callback `listen` was registered with, which is what an `Emitter::emit` from Rust amounts to. */
const delivering = () => {
    const registered = listened.mock.calls[0]?.[1];
    if (typeof registered !== "function") throw new Error("nothing subscribed to the progress event");

    return registered as (event: Event<ExportProgress>) => void;
};

describe("onExportProgress", () => {
    it("subscribes to the export's own event, not the canvas's", () => {
        onExportProgress(() => {});

        expect(listened).toHaveBeenCalledWith("export:progress", expect.any(Function));
        expect(listened).not.toHaveBeenCalledWith("enhance:progress", expect.anything());
    });

    it("hands the handler every report unchanged", () => {
        const seen: ExportProgress[] = [];
        onExportProgress((report) => seen.push(report));

        const reports: ExportProgress[] = [
            {
                run: "session-1",
                phase: "enhancing",
                operation: "Kyoto 2x (FP32)",
                family: "upscale",
                stage: "installing",
                chainFraction: 0.02,
                installFraction: 0.41,
            },
            { run: "session-1", phase: "enhancing", operation: "Kyoto 2x (FP32)", family: "upscale", chainFraction: 1 },
            { run: "session-1", phase: "writing" },
        ];
        for (const [id, payload] of reports.entries()) {
            delivering()({ event: "export:progress", id, payload });
        }

        expect(seen).toEqual(reports);
    });
});

describe("pickDirectory", () => {
    it("calls the command Rust registers, by name, with the translated title", async () => {
        invoked.mockResolvedValue("/Users/someone/Exports");

        await expect(pickDirectory("Choose a folder")).resolves.toBe("/Users/someone/Exports");
        expect(invoked).toHaveBeenCalledWith("pick_directory", { title: "Choose a folder" });
    });

    it("answers a dismissal as no folder rather than as a failure", async () => {
        invoked.mockResolvedValue(null);

        await expect(pickDirectory("Choose a folder")).resolves.toBeUndefined();
    });
});

describe("revealExport", () => {
    it("calls the command Rust registers, by name, with the path the export answered", async () => {
        invoked.mockResolvedValue(undefined);

        await revealExport("/Users/someone/Exports/holiday-opai_1.jpg");

        expect(invoked).toHaveBeenCalledWith("reveal_export", { path: "/Users/someone/Exports/holiday-opai_1.jpg" });
    });

    it("propagates the refusal", async () => {
        const refusal = { kind: "revealExport", message: "the file is not one this session has exported" };
        invoked.mockRejectedValue(refusal);

        await expect(revealExport("/etc/passwd")).rejects.toEqual(refusal);
    });
});
