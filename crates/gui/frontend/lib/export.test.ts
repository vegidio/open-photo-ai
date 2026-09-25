import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import type { Operation } from "@/ipc/enhance";
import type { ImageRecord } from "@/ipc/images";
import {
    describeExportError,
    destinationFor,
    exportNameFor,
    extensionFor,
    FORMAT_CHOICES,
    formatFor,
    groupByChain,
    queueQualityFormat,
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
        expect(formatFor(record(path), "preserve")).toBe(format);
        expect(extensionFor(record(path), "preserve")).toBe(extension);
    });

    it("Preserve writes a RAW source as TIFF", () => {
        expect(formatFor(record("/photos/shot.nef"), "preserve")).toBe("tiff");
        expect(extensionFor(record("/photos/shot.nef"), "preserve")).toBe("tiff");
    });

    it("Preserve writes a source with no extension as TIFF", () => {
        expect(extensionFor(record("/photos/shot"), "preserve")).toBe("tiff");
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
        expect(formatFor(record("/photos/shot.nef"), choice)).toBe(choice);
        expect(extensionFor(record("/photos/shot.nef"), choice)).toBe(extension);
    });

    it("offers Preserve and the eight formats", () => {
        expect(FORMAT_CHOICES).toEqual(["preserve", "avif", "bmp", "gif", "heic", "jpeg", "png", "tiff", "webp"]);
    });
});

describe("destinationFor", () => {
    it("puts the affixes around the name and writes into the source's directory", () => {
        const choices = { prefix: "new-", suffix: "-opai" };

        expect(exportNameFor(record("/photos/beach.jpg"), choices, "webp")).toBe("new-beach-opai.webp");
        expect(destinationFor(record("/photos/beach.jpg"), choices, "webp")).toBe("/photos/new-beach-opai.webp");
    });

    it("keeps a Windows path in backslashes", () => {
        expect(destinationFor(record("C:\\Users\\someone\\beach.JPG"), unaffixed, "png")).toBe(
            "C:\\Users\\someone\\beach.png",
        );
    });

    it("cuts only the extension from a name with dots in it", () => {
        expect(destinationFor(record("/photos/2026.09.24 beach.jpg"), unaffixed, "preserve")).toBe(
            "/photos/2026.09.24 beach.jpg",
        );
    });

    it("writes into a browsed folder instead", () => {
        expect(destinationFor(record("/photos/beach.jpg"), { ...unaffixed, location: "/exports" }, "png")).toBe(
            "/exports/beach.png",
        );
        expect(destinationFor(record("/photos/beach.jpg"), { ...unaffixed, location: "D:\\Exports\\" }, "png")).toBe(
            "D:\\Exports\\beach.png",
        );
    });

    it("writes a file at the root into the root", () => {
        expect(destinationFor(record("/beach.jpg"), unaffixed, "png")).toBe("/beach.png");
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
    const upscale: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 };
    const bigger: Operation = { ...upscale, scale: 4 };
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

describe("queueQualityFormat", () => {
    it("is the one lossy format a uniform queue is written in", () => {
        expect(queueQualityFormat([record("/a.png"), record("/b.nef")], "webp")).toBe("webp");
        expect(queueQualityFormat([record("/a.jpg"), record("/b.jpeg")], "preserve")).toBe("jpeg");
        expect(queueQualityFormat([record("/a.heic"), record("/b.heif")], "preserve")).toBe("heic");
    });

    it("is nothing for a mixed Preserve", () => {
        expect(queueQualityFormat([record("/a.jpg"), record("/b.png")], "preserve")).toBeUndefined();
        expect(queueQualityFormat([record("/a.jpg"), record("/b.webp")], "preserve")).toBeUndefined();
    });

    it("is nothing for a lossless format, and for an empty queue", () => {
        expect(queueQualityFormat([record("/a.jpg")], "png")).toBeUndefined();
        expect(queueQualityFormat([record("/a.nef")], "preserve")).toBeUndefined();
        expect(queueQualityFormat([], "jpeg")).toBeUndefined();
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
