import { act } from "@testing-library/react";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { analyse } from "@/hooks/useAutopilot";
import "@/i18n";
import { cancelSuggest, type Suggestion, suggest } from "@/ipc/autopilot";
import type { CropInfo } from "@/ipc/crop";
import type { Operation } from "@/ipc/enhance";
import { cancelExport, type Exported, type ExportRequest, exportImage } from "@/ipc/export";
import { detectFaces, type Face } from "@/ipc/faces";
import type { ImageRecord } from "@/ipc/images";
import { faceKey } from "@/lib/faces";
import { track } from "@/lib/faro";
import { useAutopilotStore } from "@/stores/autopilot";
import { useEnhancementStore } from "@/stores/enhancements";
import { type Row, useExportBatchStore } from "@/stores/exportBatch";
import { type ExportSettingsData, exportSettingsDefaults } from "@/stores/exportSettings";
import { useFacesStore } from "@/stores/faces";
import type { QualityChoices } from "@/stores/settings";
import {
    APPLY_ORDER,
    CATALOGUE,
    frame,
    FRAMING,
    HOLIDAY,
    openFiles,
    PUBLISHED_QUALITY,
    resetCropStore,
    resetFileStore,
    resetSettingsStore,
    SUNSET,
} from "@/test/support";
import { routeProgress, runBatch } from "./batch";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

vi.mock("@/ipc/export", async () => {
    const { EXPORT_FORMATS } = await import("@/test/support");

    return {
        exportImage: vi.fn(),
        cancelExport: vi.fn(() => Promise.resolve()),
        exportFormats: vi.fn(() => Promise.resolve(EXPORT_FORMATS)),
    };
});
vi.mock("@/ipc/autopilot", () => ({ suggest: vi.fn(), cancelSuggest: vi.fn(() => Promise.resolve()) }));
// Mocked only to say it is never reached: an export finds its own faces.
vi.mock("@/ipc/faces", () => ({ detectFaces: vi.fn() }));
vi.mock("@/ipc/catalogue", () => ({ catalogue: vi.fn(() => Promise.resolve(CATALOGUE)) }));
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

const exported = exportImage as unknown as Mock;
const stopped = cancelExport as unknown as Mock;
const asked = suggest as unknown as Mock;
const withdrawn = cancelSuggest as unknown as Mock;
const detected = detectFaces as unknown as Mock;

/** A promise the test settles by hand. */
const deferred = <T>() => {
    let resolve: (value: T) => void = () => {};
    let reject: (error: unknown) => void = () => {};
    const promise = new Promise<T>((yes, no) => {
        resolve = yes;
        reject = no;
    });

    return { promise, resolve, reject };
};

/** Lets every promise chain the settled one started run to its end. */
const settle = () => act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));

type Exporting = {
    run: string;
    source: string;
    operations: Operation[];
    request: ExportRequest;
    crop?: CropInfo;
    answer: (answer: Exported) => Promise<void>;
    fail: (error: unknown) => Promise<void>;
};

const exporting: Exporting[] = [];
const asking: { answer: (suggestions: Suggestion[]) => Promise<void>; fail: (e: unknown) => Promise<void> }[] = [];

/** A third photograph, a JPEG, for the batches that need one more than the support module holds. */
const THIRD: ImageRecord = {
    ...HOLIDAY,
    path: "/Users/someone/Pictures/third.jpg",
    identity: "1111111111111111",
    extension: "jpg",
};

const upscale: Operation = { family: "upscale", codename: "kyoto", precision: "fp32", parameters: { scale: 2 } };
const recovery = { family: "face_recovery", codename: "athens", precision: "fp32", parameters: {} } as Operation;

const face = (left: number, edge: number): Face => ({
    bounding_box: { min: { x: left, y: 0 }, max: { x: left + edge, y: edge } },
    landmarks: [
        { x: left + 1, y: 1 },
        { x: left + 2, y: 1 },
        { x: left + 1.5, y: 2 },
        { x: left + 1, y: 2.5 },
        { x: left + 2, y: 2.5 },
    ],
    confidence: 0.9,
    // What the backend publishes for such a box: `opai`'s `Face::restorable`, at most 512x512.
    restorable: edge * edge <= 512 * 512,
    key: `${left},0,${left + edge},${edge}`,
});

const settings = (changes: Partial<ExportSettingsData> = {}): ExportSettingsData => ({
    ...exportSettingsDefaults(),
    ...changes,
});

const batch = () => useExportBatchStore.getState();
const row = (file: ImageRecord): Row | undefined => batch().rows.get(file.path);
const stages = () => batch().queue.map((path) => batch().rows.get(path)?.stage);
const stack = (file: ImageRecord, operations: Operation[]) =>
    useEnhancementStore.getState().addEnhancements(file.path, operations, APPLY_ORDER);

/** Opens the dialog's queue over `files` and runs it, answering the run's promise. */
const run = (files: ImageRecord[], chosen = settings(), quality: QualityChoices = { ...PUBLISHED_QUALITY }) => {
    batch().open(files.map((file) => file.path));

    return runBatch(chosen, quality);
};

beforeEach(() => {
    vi.clearAllMocks();
    exporting.length = 0;
    asking.length = 0;

    let exports = 0;
    exported.mockImplementation(
        (source: string, operations: Operation[], _processor: string, request: ExportRequest, crop?: CropInfo) => {
            const { promise, resolve, reject } = deferred<Exported>();
            const name = `export-${++exports}`;

            exporting.push({
                run: name,
                source,
                operations,
                request,
                ...(crop && { crop }),
                answer: async (answer) => {
                    resolve(answer);
                    await settle();
                },
                fail: async (error) => {
                    reject(error);
                    await settle();
                },
            });

            return { run: name, done: promise };
        },
    );

    let suggestions = 0;
    asked.mockImplementation(() => {
        const { promise, resolve, reject } = deferred<Suggestion[]>();
        asking.push({
            answer: async (answer) => {
                resolve(answer);
                await settle();
            },
            fail: async (error) => {
                reject(error);
                await settle();
            },
        });

        return { run: `suggest-${++suggestions}`, done: promise };
    });

    localStorage.clear();
    resetSettingsStore();
    resetCropStore();
    useEnhancementStore.setState({ autopilot: false, enhancements: new Map() });
    useFacesStore.setState(useFacesStore.getInitialState(), true);
    useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);
    useExportBatchStore.setState(useExportBatchStore.getInitialState(), true);

    resetFileStore();
    openFiles(HOLIDAY, SUNSET, THIRD);
});

describe("the order", () => {
    it("exports in queue order, one file at a time, and continues past a failed file", async () => {
        const done = run([SUNSET, HOLIDAY, THIRD]);
        await settle();

        expect(exporting.map(({ source }) => source)).toEqual([SUNSET.identity]);
        expect(stages()).toEqual(["enhancing", "queued", "queued"]);

        await exporting[0]?.fail({ kind: "write", path: "/x.png", message: "Permission denied" });
        expect(exporting.map(({ source }) => source)).toEqual([SUNSET.identity, HOLIDAY.identity]);

        await exporting[1]?.answer({ outcome: "exported", path: "/Users/someone/Pictures/holiday_1.png", bytes: 10 });
        await exporting[2]?.answer({ outcome: "exported", path: "/Users/someone/Pictures/third.png", bytes: 20 });
        await done;

        expect(stages()).toEqual(["failed", "done", "done"]);
        expect(row(HOLIDAY)).toEqual({ stage: "done", path: "/Users/someone/Pictures/holiday_1.png", bytes: 10 });
        expect(batch().phase).toBe("ended");

        // Run to the end, a failed file included: `exported` counts what was written.
        expect(vi.mocked(track).mock.calls).toEqual([
            ["export_started", { file_count: 3, format: "png", processor: "auto" }],
            ["export_finished", { file_count: 3, exported: 2, completed: true, duration_ms: expect.any(Number) }],
        ]);
    });

    it("writes each file at its destination, in the chosen format", async () => {
        void run([HOLIDAY], settings({ prefix: "new-", suffix: "-opai", location: "/exports", format: "webp" }));
        await settle();

        expect(exporting[0]?.request).toMatchObject({
            destination: "/exports/new-holiday-opai.webp",
            format: "webp",
            overwrite: false,
        });
    });

    it("sends the photograph's framing", async () => {
        frame(HOLIDAY, FRAMING);

        void run([HOLIDAY]);
        await settle();

        expect(exporting[0]?.crop).toBe(FRAMING);
    });

    it("says why a file failed, naming the file and folder of a write", async () => {
        const done = run([HOLIDAY]);
        await settle();
        await exporting[0]?.fail({ kind: "write", path: "/x.png", message: "Permission denied (os error 13)" });
        await done;

        expect(row(HOLIDAY)).toEqual({
            stage: "failed",
            reason: "Couldn't write holiday.png to /Users/someone/Pictures. Permission denied (os error 13)",
        });
    });

    it("fails a record with no identity without asking for anything, and goes on", async () => {
        const unreadable: ImageRecord = { path: "/Users/someone/Pictures/broken.jpg", extension: "jpg" };
        openFiles(unreadable);

        const done = run([unreadable, HOLIDAY]);
        await settle();
        await exporting[0]?.answer({ outcome: "exported", path: "/h.png", bytes: 1 });
        await done;

        expect(row(unreadable)).toEqual({ stage: "failed", reason: "Couldn't read this image." });
        expect(exporting.map(({ source }) => source)).toEqual([HOLIDAY.identity]);
    });
});

describe("Abort", () => {
    it("during enhancing stops the export, leaves its row and the rest Queued, and starts nothing more", async () => {
        const done = run([HOLIDAY, SUNSET, THIRD]);
        await settle();
        await exporting[0]?.answer({ outcome: "exported", path: "/h.png", bytes: 1 });

        batch().abort();
        expect(stopped).toHaveBeenCalledExactlyOnceWith(exporting[1]?.run);

        await exporting[1]?.answer({ outcome: "stopped" });
        await done;

        expect(stages()).toEqual(["done", "queued", "queued"]);
        expect(exporting).toHaveLength(2);
        expect(batch().phase).toBe("ended");

        expect(track).toHaveBeenLastCalledWith("export_finished", {
            file_count: 3,
            exported: 1,
            completed: false,
            duration_ms: expect.any(Number),
        });
    });

    it("keeps Done for a file the stop reached only once it was being written", async () => {
        const done = run([HOLIDAY, SUNSET]);
        await settle();
        routeProgress({ run: exporting[0]?.run ?? "", phase: "writing" });

        batch().abort();
        await exporting[0]?.answer({ outcome: "exported", path: "/h.png", bytes: 1 });
        await done;

        expect(stages()).toEqual(["done", "queued"]);
        expect(exporting).toHaveLength(1);
    });

    it("during an Autopilot analysis stops it, leaves the row Queued and exports nothing", async () => {
        useEnhancementStore.getState().setAutopilot(true);

        const done = run([SUNSET, HOLIDAY]);
        await settle();
        expect(row(SUNSET)).toEqual({ stage: "analysing" });

        batch().abort();
        expect(withdrawn).toHaveBeenCalledExactlyOnceWith("suggest-1");

        await asking[0]?.fail({ kind: "stopped" });
        await done;

        expect(stages()).toEqual(["queued", "queued"]);
        expect(exporting).toHaveLength(0);
        expect(useEnhancementStore.getState().enhancements.has(SUNSET.path)).toBe(false);
    });
});

describe("Autopilot at export", () => {
    it("analyses a photograph with no stack, writes its suggestions into the stack and sends them as the chain", async () => {
        useEnhancementStore.getState().setAutopilot(true);

        const done = run([SUNSET]);
        await settle();
        await asking[0]?.answer([{ family: "upscale", scale: 2 }]);

        const written = useEnhancementStore.getState().enhancements.get(SUNSET.path);
        expect(written?.map(({ family }) => family)).toEqual(["upscale"]);
        expect(exporting[0]?.operations).toEqual(written);
        expect(row(SUNSET)).toEqual({ stage: "enhancing", fraction: 0 });

        await exporting[0]?.answer({ outcome: "exported", path: "/s.png", bytes: 1 });
        await done;
    });

    it("exports a photograph with no stack as it is with Autopilot off, asking for no analysis", async () => {
        void run([SUNSET]);
        await settle();

        expect(asked).not.toHaveBeenCalled();
        expect(exporting[0]?.operations).toEqual([]);
    });

    it("exports as it is where the analysis suggests nothing", async () => {
        useEnhancementStore.getState().setAutopilot(true);

        void run([SUNSET]);
        await settle();
        await asking[0]?.answer([]);

        expect(exporting[0]?.operations).toEqual([]);
    });

    it("reads a failed analysis as Failed, with no notice, and goes on", async () => {
        useEnhancementStore.getState().setAutopilot(true);
        stack(HOLIDAY, [upscale]);
        const notice = vi.spyOn(toast, "error");
        vi.spyOn(console, "error").mockImplementation(() => {});

        const done = run([SUNSET, HOLIDAY]);
        await settle();
        await asking[0]?.fail({ kind: "analyse", message: "the colour signal could not be read" });

        expect(row(SUNSET)).toEqual({
            stage: "failed",
            reason: "Autopilot couldn't analyse this image. the colour signal could not be read",
        });
        expect(notice).not.toHaveBeenCalled();
        expect(exporting.map(({ source }) => source)).toEqual([HOLIDAY.identity]);

        await exporting[0]?.answer({ outcome: "exported", path: "/h.png", bytes: 1 });
        await done;
    });

    it("waits on an analysis already in flight rather than asking for a second, and exports what it adds", async () => {
        useEnhancementStore.getState().setAutopilot(true);
        // The canvas trigger's own analysis, under way before Save is pressed.
        void analyse(SUNSET, undefined, { notify: true });
        await settle();

        const done = run([SUNSET]);
        await settle();

        expect(asked).toHaveBeenCalledOnce();
        expect(row(SUNSET)).toEqual({ stage: "analysing" });
        expect(exporting).toHaveLength(0);

        await asking[0]?.answer([{ family: "upscale", scale: 2 }]);

        expect(asked).toHaveBeenCalledOnce();
        expect(exporting[0]?.operations).toEqual(useEnhancementStore.getState().enhancements.get(SUNSET.path));
        expect(exporting[0]?.operations.map(({ family }) => family)).toEqual(["upscale"]);

        await exporting[0]?.answer({ outcome: "exported", path: "/s.png", bytes: 1 });
        await done;
    });

    it("leaves a photograph that already has a stack alone", async () => {
        useEnhancementStore.getState().setAutopilot(true);
        stack(HOLIDAY, [upscale]);

        void run([HOLIDAY]);
        await settle();

        expect(asked).not.toHaveBeenCalled();
        expect(exporting[0]?.operations).toEqual([upscale]);
    });
});

describe("the faces", () => {
    it("are never detected first: the export is asked for at once, carrying the choice made among them", async () => {
        // The export finds its own faces inside the one request, under its stop and on its bar.
        const choice = { skipped: [faceKey(face(0, 40))], restored: [] };
        useFacesStore.getState().setFaceChoice(HOLIDAY.identity ?? "", choice);
        stack(HOLIDAY, [upscale, recovery]);

        void run([HOLIDAY]);
        await settle();

        expect(detected).not.toHaveBeenCalled();
        expect(exporting[0]?.operations).toEqual([{ ...recovery, faces: choice }, upscale]);
    });

    it("carry no choice where nobody made one, so every face follows its default", async () => {
        stack(HOLIDAY, [recovery]);

        void run([HOLIDAY]);
        await settle();

        expect(exporting[0]?.operations).toEqual([recovery]);
    });
});

describe("the quality", () => {
    it("is the committed one for every file", async () => {
        const done = run([HOLIDAY, SUNSET], settings({ format: "webp" }), { ...PUBLISHED_QUALITY, webp: 70 });
        await settle();
        await exporting[0]?.answer({ outcome: "exported", path: "/h.webp", bytes: 1 });
        await exporting[1]?.answer({ outcome: "exported", path: "/s.webp", bytes: 1 });
        await done;

        expect(exporting.map(({ request }) => request.quality)).toEqual([70, 70]);
    });

    it("is each file's own format's under a mixed Preserve", async () => {
        const quality = { ...PUBLISHED_QUALITY, jpeg: 81, heic: 42 };

        const done = run([THIRD, SUNSET], settings({ format: "preserve" }), quality);
        await settle();
        await exporting[0]?.answer({ outcome: "exported", path: "/t.jpg", bytes: 1 });
        await exporting[1]?.answer({ outcome: "exported", path: "/s.tiff", bytes: 1 });
        await done;

        expect(exporting.map(({ request }) => [request.format, request.quality])).toEqual([
            ["jpeg", 81],
            // A RAW source is written as TIFF, which takes no quality, and is sent none.
            ["tiff", undefined],
        ]);
        expect("quality" in (exporting[1]?.request ?? {})).toBe(false);
    });

    it("is sent for no format the user never moved, which Rust writes at its published default", async () => {
        const done = run([HOLIDAY], settings({ format: "webp" }), {});
        await settle();
        await exporting[0]?.answer({ outcome: "exported", path: "/h.webp", bytes: 1 });
        await done;

        expect(exporting[0]?.request).not.toHaveProperty("quality");
        expect(exporting[0]?.request.format).toBe("webp");
    });
});

describe("the progress", () => {
    it("moves the row being exported, and ignores a report for any other run", async () => {
        void run([HOLIDAY]);
        await settle();
        const name = exporting[0]?.run ?? "";

        routeProgress({
            run: "another",
            phase: "enhancing",
            operation: "Kyoto",
            family: "upscale",
            chainFraction: 0.9,
        });
        expect(row(HOLIDAY)).toEqual({ stage: "enhancing", fraction: 0 });

        routeProgress({ run: name, phase: "enhancing", operation: "Kyoto", family: "upscale", chainFraction: 0.4 });
        expect(row(HOLIDAY)).toEqual({ stage: "enhancing", fraction: 0.4 });

        routeProgress({ run: name, phase: "writing" });
        expect(row(HOLIDAY)).toEqual({ stage: "writing" });

        await exporting[0]?.answer({ outcome: "exported", path: "/h.png", bytes: 3 });
        routeProgress({ run: name, phase: "writing" });

        expect(row(HOLIDAY)).toEqual({ stage: "done", path: "/h.png", bytes: 3 });
    });
});
