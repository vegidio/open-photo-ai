import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import type { Operation } from "@/ipc/enhance";
import type { ImageRecord } from "@/ipc/images";
import { EXPORT_FORMATS } from "@/test/support";
import {
    describeExportError,
    destinationFor,
    exportNameFor,
    extensionFor,
    FORMAT_CHOICES,
    formatFor,
    groupByChain,
    qualityFor,
    queueQualityFor,
} from "./export";
import { directoryOf } from "./paths";

const record = (path: string): ImageRecord => ({
    path,
    identity: "0123456789abcdef",
    width: 1000,
    height: 800,
    extension: path.split(".").pop()?.toLowerCase() ?? "",
    size: 1024,
});

const unaffixed = { prefix: "", suffix: "" };

/** A `t` that shows which key it was asked for and with what, so a test reads the composition rather than English. */
const t = ((key: string, values?: Record<string, unknown>) =>
    values ? `${key}(${JSON.stringify(values)})` : key) as unknown as TFunction;

describe("formatFor and extensionFor", () => {
    it.each([
        ["/photos/beach.jpg", "jpeg", "jpg"],
        ["/photos/beach.JPEG", "jpeg", "jpeg"],
        ["/photos/beach.png", "png", "png"],
        ["/photos/beach.heif", "heic", "heif"],
        ["/photos/beach.heic", "heic", "heic"],
        ["/photos/beach.tif", "tiff", "tif"],
    ])("Preserve keeps %s as its own format and extension", (path, format, extension) => {
        expect(formatFor(record(path), "preserve", EXPORT_FORMATS)).toBe(format);
        expect(extensionFor(record(path), "preserve", EXPORT_FORMATS)).toBe(extension);
    });

    it("Preserve writes a RAW source as TIFF", () => {
        expect(formatFor(record("/photos/shot.nef"), "preserve", EXPORT_FORMATS)).toBe("tiff");
        expect(extensionFor(record("/photos/shot.nef"), "preserve", EXPORT_FORMATS)).toBe("tiff");
    });

    it("Preserve writes a source with no extension as TIFF", () => {
        expect(extensionFor(record("/photos/shot"), "preserve", EXPORT_FORMATS)).toBe("tiff");
    });

    it.each([
        ["avif", "avif"],
        ["bmp", "bmp"],
        ["gif", "gif"],
        ["heic", "heic"],
        ["jpeg", "jpg"],
        ["png", "png"],
        ["tiff", "tiff"],
        ["webp", "webp"],
    ] as const)("a chosen %s writes its own format with the extension %s, whatever the source", (choice, extension) => {
        expect(formatFor(record("/photos/shot.nef"), choice, EXPORT_FORMATS)).toBe(choice);
        expect(extensionFor(record("/photos/shot.nef"), choice, EXPORT_FORMATS)).toBe(extension);
    });

    it("offers Preserve and the eight formats", () => {
        expect(FORMAT_CHOICES).toEqual(["preserve", "avif", "bmp", "gif", "heic", "jpeg", "png", "tiff", "webp"]);
    });
});

describe("destinationFor", () => {
    it("puts the affixes around the name and writes into the source's directory", () => {
        const choices = { prefix: "new-", suffix: "-opai" };

        expect(exportNameFor(record("/photos/beach.jpg"), choices, "webp", EXPORT_FORMATS)).toBe("new-beach-opai.webp");
        expect(destinationFor(record("/photos/beach.jpg"), choices, "webp", EXPORT_FORMATS)).toBe(
            "/photos/new-beach-opai.webp",
        );
    });

    it("keeps a Windows path in backslashes", () => {
        expect(destinationFor(record("C:\\Users\\someone\\beach.JPG"), unaffixed, "png", EXPORT_FORMATS)).toBe(
            "C:\\Users\\someone\\beach.png",
        );
    });

    it("cuts only the extension from a name with dots in it", () => {
        expect(destinationFor(record("/photos/2026.09.24 beach.jpg"), unaffixed, "preserve", EXPORT_FORMATS)).toBe(
            "/photos/2026.09.24 beach.jpg",
        );
    });

    it("writes into a browsed folder instead", () => {
        expect(
            destinationFor(record("/photos/beach.jpg"), { ...unaffixed, location: "/exports" }, "png", EXPORT_FORMATS),
        ).toBe("/exports/beach.png");
        expect(
            destinationFor(
                record("/photos/beach.jpg"),
                { ...unaffixed, location: "D:\\Exports\\" },
                "png",
                EXPORT_FORMATS,
            ),
        ).toBe("D:\\Exports\\beach.png");
    });

    it("writes a file at the root into the root", () => {
        expect(destinationFor(record("/beach.jpg"), unaffixed, "png", EXPORT_FORMATS)).toBe("/beach.png");
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
        expect(qualityFor(record("/a.png"), "jpeg", EXPORT_FORMATS)).toEqual({
            format: "jpeg",
            range: { min: 1, max: 100, default: 90 },
        });
        expect(qualityFor(record("/a.jpg"), "png", EXPORT_FORMATS)).toBeUndefined();
    });

    it("reads which formats take one off what Rust publishes, naming none of its own", () => {
        const noneLossy = {
            ...EXPORT_FORMATS,
            formats: EXPORT_FORMATS.formats.map((format) => ({ ...format, quality: null })),
        };

        expect(qualityFor(record("/a.jpg"), "jpeg", noneLossy)).toBeUndefined();
    });
});

describe("queueQualityFor", () => {
    it("is the one lossy format a uniform queue is written in", () => {
        expect(queueQualityFor([record("/a.png"), record("/b.nef")], "webp", EXPORT_FORMATS)?.format).toBe("webp");
        expect(queueQualityFor([record("/a.jpg"), record("/b.jpeg")], "preserve", EXPORT_FORMATS)?.format).toBe("jpeg");
        expect(queueQualityFor([record("/a.heic"), record("/b.heif")], "preserve", EXPORT_FORMATS)?.format).toBe(
            "heic",
        );
    });

    it("is nothing for a mixed Preserve", () => {
        expect(queueQualityFor([record("/a.jpg"), record("/b.png")], "preserve", EXPORT_FORMATS)).toBeUndefined();
        expect(queueQualityFor([record("/a.jpg"), record("/b.webp")], "preserve", EXPORT_FORMATS)).toBeUndefined();
    });

    it("is nothing for a lossless format, and for an empty queue", () => {
        expect(queueQualityFor([record("/a.jpg")], "png", EXPORT_FORMATS)).toBeUndefined();
        expect(queueQualityFor([record("/a.nef")], "preserve", EXPORT_FORMATS)).toBeUndefined();
        expect(queueQualityFor([], "jpeg", EXPORT_FORMATS)).toBeUndefined();
    });
});

describe("describeExportError", () => {
    const destination = "/exports/beach-opai.png";

    it("names the file and the folder of a write, then the operating system's reason untranslated", () => {
        const error = { kind: "write", path: destination, message: "Permission denied (os error 13)" };

        expect(describeExportError(t, { cause: "export", error }, destination)).toBe(
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
        expect(describeExportError(t, { cause: "export", error }, destination)).toBe(
            `export.queue.enhanceFailed ${detail}`,
        );
    });

    it("says a record with no identity could not be read", () => {
        expect(describeExportError(t, { cause: "unreadable" }, destination)).toBe("export.queue.unreadable");
    });

    it("says the analysis failed, with its message where the rejection carries one", () => {
        const error = { kind: "analyse", message: "the detector failed" };

        expect(describeExportError(t, { cause: "analysis", error }, destination)).toBe(
            "export.queue.analysisFailed the detector failed",
        );
        expect(describeExportError(t, { cause: "analysis" }, destination)).toBe("export.queue.analysisFailed");
    });
});
