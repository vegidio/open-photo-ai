import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import type { Operation } from "@/ipc/enhance";
import { EXPORT_FORMATS } from "@/test/support";
import { describeExportError, FORMAT_CHOICES, groupByChain, qualityFor, queueQualityFor } from "./export";
import { directoryOf } from "./paths";

/** A `t` that shows which key it was asked for and with what, so a test reads the composition rather than English. */
const t = ((key: string, values?: Record<string, unknown>) =>
    values ? `${key}(${JSON.stringify(values)})` : key) as unknown as TFunction;

describe("FORMAT_CHOICES", () => {
    it("offers Preserve and the eight formats", () => {
        expect(FORMAT_CHOICES).toEqual(["preserve", "avif", "bmp", "gif", "heic", "jpeg", "png", "tiff", "webp"]);
    });
});

describe("directoryOf", () => {
    it.each([
        ["/photos/beach.jpg", "/photos"],
        ["C:\\Users\\someone\\beach.jpg", "C:\\Users\\someone"],
        ["/beach.jpg", "/"],
        ["beach.jpg", ""],
    ])("the directory of %s is %s", (path, directory) => {
        expect(directoryOf(path)).toBe(directory);
    });
});

describe("groupByChain", () => {
    const upscale: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", parameters: { scale: 2 } };
    const bigger: Operation = { ...upscale, parameters: { scale: 4 } };
    const denoise = { family: "denoise", codename: "stockholm", precision: "fp32" } as Operation;

    it("puts stacks running the same families together, groups in the order of their first member", () => {
        const stacks = new Map<string, Operation[]>([
            ["A", [upscale]],
            ["B", [denoise, upscale]],
            ["C", [bigger]],
            ["D", [denoise, upscale]],
        ]);

        expect(groupByChain(["A", "B", "C", "D"], (path) => stacks.get(path))).toEqual(["A", "C", "B", "D"]);
    });

    it("groups paths with no stack or an empty one together, keeping drawer order within every group", () => {
        const stacks = new Map<string, Operation[]>([
            ["B", [upscale]],
            ["C", []],
            ["E", [upscale]],
        ]);

        expect(groupByChain(["A", "B", "C", "D", "E"], (path) => stacks.get(path))).toEqual(["A", "C", "D", "B", "E"]);
    });

    it("leaves a uniform queue in its order", () => {
        expect(groupByChain(["C", "A", "B"], () => [upscale])).toEqual(["C", "A", "B"]);
    });
});

describe("qualityFor", () => {
    it("is the written format's published range, for a format that takes one", () => {
        expect(qualityFor("jpeg", EXPORT_FORMATS)).toEqual({
            format: "jpeg",
            range: { min: 1, max: 100, default: 90 },
        });
        expect(qualityFor("png", EXPORT_FORMATS)).toBeUndefined();
    });

    it("reads which formats take one off what Rust publishes, naming none of its own", () => {
        const noneLossy = {
            ...EXPORT_FORMATS,
            formats: EXPORT_FORMATS.formats.map((format) => ({ ...format, quality: null })),
        };

        expect(qualityFor("jpeg", noneLossy)).toBeUndefined();
    });
});

describe("queueQualityFor", () => {
    it("is the one lossy format a uniform queue is written in", () => {
        expect(queueQualityFor(["webp", "webp"], EXPORT_FORMATS)?.format).toBe("webp");
        // Under Preserve, `.jpg` and `.jpeg` are both written as JPEG, and `.heic` and `.heif` both as HEIC.
        expect(queueQualityFor(["jpeg", "jpeg"], EXPORT_FORMATS)?.format).toBe("jpeg");
        expect(queueQualityFor(["heic", "heic"], EXPORT_FORMATS)?.format).toBe("heic");
    });

    it("is nothing for a mixed Preserve", () => {
        expect(queueQualityFor(["jpeg", "png"], EXPORT_FORMATS)).toBeUndefined();
        expect(queueQualityFor(["jpeg", "webp"], EXPORT_FORMATS)).toBeUndefined();
    });

    it("is nothing for a lossless format, and for an empty queue", () => {
        expect(queueQualityFor(["png"], EXPORT_FORMATS)).toBeUndefined();
        // A RAW source under Preserve, written as TIFF.
        expect(queueQualityFor(["tiff"], EXPORT_FORMATS)).toBeUndefined();
        expect(queueQualityFor([], EXPORT_FORMATS)).toBeUndefined();
    });
});

describe("describeExportError", () => {
    it("names the file and the folder of a write from the error's own path, then the operating system's reason untranslated", () => {
        const error = { kind: "write", path: "/exports/beach-opai.png", message: "Permission denied (os error 13)" };

        expect(describeExportError(t, { cause: "export", error })).toBe(
            'export.queue.writeFailed({"name":"beach-opai.png","folder":"/exports"}) Permission denied (os error 13)',
        );
    });

    it.each([
        [{ kind: "enhance", message: "the model could not be downloaded" }, "the model could not be downloaded"],
        [{ kind: "unreadableSource", identity: "0123", message: "no such file" }, "no such file"],
        [{ kind: "notReady" }, "notReady"],
        [{ kind: "unknownSource", identity: "0123" }, "unknownSource"],
        [{ kind: "unknownOperation", index: 0, reason: { kind: "model" } }, "unknownOperation"],
    ])("says an enhancement failed, with the library's sentence or the kind: %j", (error, detail) => {
        expect(describeExportError(t, { cause: "export", error })).toBe(`export.queue.enhanceFailed ${detail}`);
    });

    it("says a record with no identity could not be read", () => {
        expect(describeExportError(t, { cause: "unreadable" })).toBe("export.queue.unreadable");
    });

    it("says the analysis failed, with its message where the rejection carries one", () => {
        const error = { kind: "analyse", message: "the detector failed" };

        expect(describeExportError(t, { cause: "analysis", error })).toBe(
            "export.queue.analysisFailed the detector failed",
        );
        expect(describeExportError(t, { cause: "analysis" })).toBe("export.queue.analysisFailed");
    });
});
