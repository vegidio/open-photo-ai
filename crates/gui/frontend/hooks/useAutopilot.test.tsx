import { act } from "@testing-library/react";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
// Initialised here, not by `AppProviders`: the failure notice is composed outside the tree.
import "@/i18n";
import { cancelSuggest, type Suggestion, suggest } from "@/ipc/autopilot";
import type { Family } from "@/ipc/catalogue";
import type { CropInfo } from "@/ipc/crop";
import { detectFaces, type Face } from "@/ipc/faces";
import { ENHANCEMENTS } from "@/lib/enhancements";
import { track } from "@/lib/faro";
import { useAutopilotStore } from "@/stores/autopilot";
import { useCropStore } from "@/stores/crop";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFacesStore } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";
import {
    CATALOGUE,
    FRAMING,
    frame,
    HOLIDAY,
    openFiles,
    render,
    resetCropStore,
    resetFileStore,
    resetSettingsStore,
    SUNSET,
} from "@/test/support";
import { analyse, awaitAnalysis, useAutopilot } from "./useAutopilot";

// The seams are pinned against the wire by their own tests; here they are mocked so this file is about what the
// window asks for and what it does with the answers.
vi.mock("@/ipc/autopilot", () => ({ suggest: vi.fn(), cancelSuggest: vi.fn(() => Promise.resolve()) }));
vi.mock("@/ipc/faces", () => ({ detectFaces: vi.fn() }));
vi.mock("@/ipc/catalogue", () => ({ catalogue: vi.fn(() => Promise.resolve(CATALOGUE)) }));

// Closing a photograph tells every owner, and the enhancement store's hands a result back to Rust.
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

const tracked = track as unknown as Mock;
const asked = suggest as unknown as Mock;
const cancelled = cancelSuggest as unknown as Mock;
const detected = detectFaces as unknown as Mock;

/** One analysis the test settles by hand. */
type Asking = {
    run: string;
    source: string;
    families: Family[];
    crop?: CropInfo;
    answer: (suggestions: Suggestion[]) => Promise<void>;
    fail: (error: unknown) => Promise<void>;
};

const asking: Asking[] = [];

/** One follow-up detection the test settles by hand. */
type Detecting = { crop?: CropInfo; found: (faces: Face[]) => Promise<void>; fail: (error: unknown) => Promise<void> };

const detecting: Detecting[] = [];

/** Lets every promise chain the settled one started run to its end, inside React's `act`. */
const settle = () => act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));

/** A square face `edge` pixels on a side. */
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
});

const stackOf = (path: string) => useEnhancementStore.getState().enhancements.get(path);
const familiesOf = (path: string) => stackOf(path)?.map((operation) => operation.family);
const inFlight = (path: string) => useAutopilotStore.getState().analysing.get(path);

/** A second framing, so a change of crop is a change of value rather than of one of its fields. */
const REFRAMED: CropInfo = { ...FRAMING, left: FRAMING.left + 200 };

const failed = () => vi.spyOn(toast, "error");

beforeEach(() => {
    vi.clearAllMocks();
    asking.length = 0;
    detecting.length = 0;

    let minted = 0;
    asked.mockImplementation((source: string, _processor: string, families: Family[], crop?: CropInfo) => {
        const run = `suggest-${++minted}`;
        let resolve: (suggestions: Suggestion[]) => void = () => {};
        let reject: (error: unknown) => void = () => {};
        const done = new Promise<Suggestion[]>((yes, no) => {
            resolve = yes;
            reject = no;
        });

        asking.push({
            run,
            source,
            families,
            ...(crop && { crop }),
            answer: async (suggestions) => {
                resolve(suggestions);
                await settle();
            },
            fail: async (error) => {
                reject(error);
                await settle();
            },
        });

        return { run, done };
    });

    let detections = 0;
    detected.mockImplementation((_source: string, _processor: string, crop?: CropInfo) => {
        const run = `detect-${++detections}`;
        let resolve: (faces: Face[]) => void = () => {};
        let reject: (error: unknown) => void = () => {};
        const done = new Promise<Face[]>((yes, no) => {
            resolve = yes;
            reject = no;
        });

        detecting.push({
            ...(crop && { crop }),
            found: async (faces) => {
                resolve(faces);
                await settle();
            },
            fail: async (error) => {
                reject(error);
                await settle();
            },
        });

        return { run, done };
    });

    localStorage.clear();
    resetSettingsStore();
    resetCropStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    useFacesStore.setState(useFacesStore.getInitialState(), true);
    useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);

    resetFileStore();
    openFiles(HOLIDAY, SUNSET);
});

describe("an analysis", () => {
    it("asks for every enhancement offered, less the ones switched off", () => {
        useSettingsStore.getState().apply({ autopilotExcluded: ["colorization", "denoise"] });

        void analyse(HOLIDAY, undefined);

        expect(asking[0]?.families).toEqual(
            ENHANCEMENTS.map(({ family }) => family).filter(
                (family) => family !== "colorization" && family !== "denoise",
            ),
        );
    });

    it("is still asked for with every enhancement switched off, and answers nothing", async () => {
        const notice = failed();
        useSettingsStore.getState().apply({ autopilotExcluded: ENHANCEMENTS.map(({ family }) => family) });

        void analyse(HOLIDAY, undefined);
        expect(asking[0]?.families).toEqual([]);

        await asking[0]?.answer([]);

        expect(stackOf(HOLIDAY.path)).toEqual([]);
        expect(notice).not.toHaveBeenCalled();
    });

    it("asks at the framing it was given", () => {
        void analyse(HOLIDAY, FRAMING);

        expect(asking[0]?.source).toBe(HOLIDAY.identity);
        expect(asking[0]?.crop).toBe(FRAMING);
        expect(inFlight(HOLIDAY.path)).toEqual({ run: "suggest-1", crop: FRAMING });
    });

    it("lands its suggestions as one batch, built as the add menu builds them, in pipeline order", async () => {
        useSettingsStore.getState().apply({ models: { colorization: "mumbai_fp16" } });
        const writes = vi.fn();
        const unsubscribe = useEnhancementStore.subscribe(writes);

        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([{ family: "upscale", scale: 4 }, { family: "colorization" }]);
        unsubscribe();

        expect(writes).toHaveBeenCalledTimes(1);
        expect(stackOf(HOLIDAY.path)).toEqual([
            { family: "colorization", codename: "mumbai", precision: "fp16" },
            { family: "upscale", codename: "tokyo", precision: "fp32", scale: 4 },
        ]);
    });

    it("ends its entry only after the stack is written", async () => {
        // What each write to the stack saw of the analysis: ending first would leave one render with neither, in
        // which the trigger would start a second analysis.
        const seen: boolean[] = [];
        const unsubscribe = useEnhancementStore.subscribe(() => seen.push(inFlight(HOLIDAY.path) !== undefined));

        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([{ family: "denoise" }]);
        unsubscribe();

        expect(seen).toEqual([true]);
        expect(inFlight(HOLIDAY.path)).toBeUndefined();
    });

    it("keeps an enhancement added by hand while it ran", async () => {
        void analyse(HOLIDAY, undefined);
        useEnhancementStore
            .getState()
            .addEnhancement(HOLIDAY.path, { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 });

        await asking[0]?.answer([{ family: "upscale", scale: 4 }, { family: "light_adjustment" }]);

        expect(stackOf(HOLIDAY.path)).toEqual([
            { family: "light_adjustment", codename: "paris", precision: "fp32", bias: 0.5 },
            { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 },
        ]);
    });

    it("sends one autopilot_run with the count, and an enhancement_added from Autopilot for each", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([{ family: "upscale", scale: 4 }, { family: "colorization" }, { family: "denoise" }]);

        // In the order Autopilot suggested them; the stack sorts them into pipeline order on its own.
        expect(tracked.mock.calls).toEqual([
            ["enhancement_added", { family: "upscale", source: "autopilot" }],
            ["enhancement_added", { family: "colorization", source: "autopilot" }],
            ["enhancement_added", { family: "denoise", source: "autopilot" }],
            ["autopilot_run", { count: 3 }],
        ]);
    });

    // A zero is an answer to how much Autopilot suggests per image: dropping it would inflate the average.
    it("sends autopilot_run with a count of zero when it suggests nothing", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([]);

        expect(tracked.mock.calls).toEqual([["autopilot_run", { count: 0 }]]);
    });

    it("sends nothing when it was stopped or failed", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});

        void analyse(HOLIDAY, undefined);
        await asking[0]?.fail({ kind: "stopped" });
        void analyse(HOLIDAY, undefined);
        await asking[1]?.fail({ kind: "analyse", message: "m" });

        expect(tracked).not.toHaveBeenCalled();
    });

    it("is applied to the photograph it was made for after the user has moved off", async () => {
        void analyse(HOLIDAY, undefined);
        useFileStore.getState().setCurrentIndex(1);

        await asking[0]?.answer([{ family: "denoise" }]);

        expect(familiesOf(HOLIDAY.path)).toEqual(["denoise"]);
        expect(stackOf(SUNSET.path)).toBeUndefined();
    });
});

describe("an analysis that does not answer", () => {
    it("says nothing and writes nothing when it was stopped", async () => {
        const notice = failed();

        void analyse(HOLIDAY, undefined);
        await asking[0]?.fail({ kind: "stopped" });

        expect(notice).not.toHaveBeenCalled();
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
        expect(inFlight(HOLIDAY.path)).toBeUndefined();
    });

    it("reports a failure, writes no stack, and declines the current photograph", async () => {
        const notice = failed();
        vi.spyOn(console, "error").mockImplementation(() => {});

        void analyse(HOLIDAY, undefined);
        await asking[0]?.fail({ kind: "analyse", message: "the colour signal could not be read" });

        expect(notice).toHaveBeenCalledExactlyOnceWith("Something went wrong. Failed to run autopilot.");
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
        expect(useAutopilotStore.getState().declined).toBe(HOLIDAY.path);
        expect(inFlight(HOLIDAY.path)).toBeUndefined();
    });

    it("declines nothing when the photograph that failed is no longer current", async () => {
        const notice = failed();
        vi.spyOn(console, "error").mockImplementation(() => {});

        void analyse(HOLIDAY, undefined);
        useFileStore.getState().setCurrentIndex(1);
        await asking[0]?.fail({ kind: "notReady" });

        expect(notice).toHaveBeenCalledOnce();
        expect("declined" in useAutopilotStore.getState()).toBe(false);
    });
});

describe("an analysis an export asks for", () => {
    it("answers added once the stack is written", async () => {
        const outcome = analyse(SUNSET, undefined, { notify: false });
        await asking[0]?.answer([{ family: "denoise" }]);

        await expect(outcome).resolves.toBe("added");
        expect(familiesOf(SUNSET.path)).toEqual(["denoise"]);
        // The export's own analysis is no decision of the user's, so it is not a usage event.
        expect(tracked).not.toHaveBeenCalled();
    });

    it("answers a failure with what it failed with, and neither notifies nor declines", async () => {
        const notice = failed();
        vi.spyOn(console, "error").mockImplementation(() => {});
        const error = { kind: "analyse", message: "the colour signal could not be read" };

        // The current photograph, which is where the canvas's own failure would decline.
        const outcome = analyse(HOLIDAY, undefined, { notify: false });
        await asking[0]?.fail(error);

        await expect(outcome).resolves.toEqual({ failed: error });
        expect(notice).not.toHaveBeenCalled();
        expect("declined" in useAutopilotStore.getState()).toBe(false);
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
    });

    it("answers stopped when it was stopped", async () => {
        const outcome = analyse(SUNSET, undefined, { notify: false });
        useAutopilotStore.getState().stop(SUNSET.path);
        await asking[0]?.answer([{ family: "denoise" }]);

        await expect(outcome).resolves.toBe("stopped");
        expect(stackOf(SUNSET.path)).toBeUndefined();
    });

    it("waits on one already in flight rather than asking a second time", async () => {
        void analyse(SUNSET, undefined);

        const outcome = awaitAnalysis(SUNSET, undefined);
        expect(asked).toHaveBeenCalledOnce();

        await asking[0]?.answer([{ family: "upscale", scale: 2 }]);

        await expect(outcome).resolves.toBe("added");
        expect(asked).toHaveBeenCalledOnce();
        expect(familiesOf(SUNSET.path)).toEqual(["upscale"]);
    });

    it("answers a failure with no cause where the one it waited on failed", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        void analyse(SUNSET, undefined);

        const outcome = awaitAnalysis(SUNSET, undefined);
        await asking[0]?.fail({ kind: "notReady" });

        await expect(outcome).resolves.toEqual({ failed: undefined });
    });

    it("asks with nothing in flight, and the trigger asks nothing more for the current photograph meanwhile", async () => {
        const outcome = awaitAnalysis(HOLIDAY, undefined);
        mount();

        expect(asked).toHaveBeenCalledOnce();

        await asking[0]?.answer([]);

        await expect(outcome).resolves.toBe("added");
        expect(asked).toHaveBeenCalledOnce();
    });
});

describe("a face-recovery suggestion", () => {
    const suggestions: Suggestion[] = [{ family: "face_recovery" }, { family: "upscale", scale: 2 }];

    it("is dropped where every face found is larger than the tile a restoration works at", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer(suggestions);
        await detecting[0]?.found([face(0, 900)]);

        expect(familiesOf(HOLIDAY.path)).toEqual(["upscale"]);
    });

    it("is kept where one face is small enough", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer(suggestions);
        await detecting[0]?.found([face(0, 300), face(1000, 900)]);

        expect(familiesOf(HOLIDAY.path)).toEqual(["face_recovery", "upscale"]);
    });

    it.each([
        ["kept", [face(0, 300)]],
        ["dropped", [face(0, 900)]],
    ])("records the faces at the framing the analysis was made at when it is %s", async (_, found) => {
        frame(HOLIDAY);
        const crop = useCropStore.getState().crops.get(HOLIDAY.identity ?? "");

        void analyse(HOLIDAY, crop);
        await asking[0]?.answer(suggestions);

        expect(detected).toHaveBeenCalledExactlyOnceWith(HOLIDAY.identity, "auto", crop);

        await detecting[0]?.found(found);

        const recorded = useFacesStore.getState().faces.get(HOLIDAY.identity ?? "");
        expect(recorded?.faces).toEqual(found);
        // The same reference, which is what `useImageFaces` matches the framing by.
        expect(recorded?.crop).toBe(crop);
    });

    it("is kept, and nothing recorded, where finding the faces fails", async () => {
        vi.spyOn(console, "warn").mockImplementation(() => {});
        const notice = failed();

        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer(suggestions);
        await detecting[0]?.fail({ kind: "detect", message: "the detector could not be read" });

        expect(familiesOf(HOLIDAY.path)).toEqual(["face_recovery", "upscale"]);
        expect(useFacesStore.getState().faces.has(HOLIDAY.identity ?? "")).toBe(false);
        expect(notice).not.toHaveBeenCalled();
    });

    it("asks for no faces where it is not among the suggestions", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([{ family: "denoise" }]);

        expect(detected).not.toHaveBeenCalled();
    });
});

describe("an answer the user has moved past", () => {
    it.each([
        ["closed", () => useFileStore.getState().closeFile(HOLIDAY.path)],
        ["switched off", () => useAutopilotStore.getState().stopAll()],
        ["reframed", () => useAutopilotStore.getState().stop(HOLIDAY.path)],
    ])("writes nothing when it arrives after the photograph was %s", async (_, moved) => {
        const notice = failed();

        void analyse(HOLIDAY, undefined);
        act(moved);
        // The backend's answer beat the stop across the boundary.
        await asking[0]?.answer([{ family: "denoise" }]);

        expect(cancelled).toHaveBeenCalledExactlyOnceWith("suggest-1");
        expect(stackOf(HOLIDAY.path)).toBeUndefined();
        expect(notice).not.toHaveBeenCalled();
    });

    it("writes nothing, faces included, when the stop lands during the follow-up detection", async () => {
        void analyse(HOLIDAY, undefined);
        await asking[0]?.answer([{ family: "face_recovery" }]);

        act(() => useAutopilotStore.getState().stop(HOLIDAY.path));
        await detecting[0]?.found([face(0, 300)]);

        expect(stackOf(HOLIDAY.path)).toBeUndefined();
        expect(useFacesStore.getState().faces.has(HOLIDAY.identity ?? "")).toBe(false);
    });

    it("says nothing about a failure that arrives after a stop", async () => {
        const notice = failed();

        void analyse(HOLIDAY, undefined);
        act(() => useAutopilotStore.getState().stopAll());
        await asking[0]?.fail({ kind: "analyse", message: "late" });

        expect(notice).not.toHaveBeenCalled();
    });
});

/** Mounts the trigger the way the sidebar does. */
const Probe = () => {
    useAutopilot();

    return null;
};

const mount = () => render(<Probe />);

describe("the trigger", () => {
    it("analyses the current photograph with Autopilot on and no stack", () => {
        mount();

        expect(asking).toHaveLength(1);
        expect(asking[0]?.source).toBe(HOLIDAY.identity);
    });

    it("never analyses a photograph that is open but not current", async () => {
        mount();
        await asking[0]?.answer([]);

        expect(asking.map((entry) => entry.source)).toEqual([HOLIDAY.identity]);
    });

    it("analyses each photograph when the user comes to it", () => {
        mount();
        act(() => useFileStore.getState().setCurrentIndex(1));

        expect(asking.map((entry) => entry.source)).toEqual([HOLIDAY.identity, SUNSET.identity]);
    });

    it("does not analyse with Autopilot off", () => {
        useEnhancementStore.setState({ autopilot: false });

        mount();

        expect(asked).not.toHaveBeenCalled();
    });

    it.each([
        ["carrying enhancements", [{ family: "denoise", codename: "stockholm", precision: "fp32", strength: 1 }]],
        ["whose enhancements were all removed", []],
    ] as const)("does not analyse a photograph %s", (_, operations) => {
        useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [...operations]]]) });

        mount();

        expect(asked).not.toHaveBeenCalled();
    });

    it("analyses the current photograph when Autopilot is turned on", () => {
        useEnhancementStore.setState({ autopilot: false });
        mount();

        act(() => useEnhancementStore.getState().setAutopilot(true));

        expect(asking).toHaveLength(1);
    });

    it("asks nothing new when the user moves off and back while it is in flight", async () => {
        mount();
        act(() => useFileStore.getState().setCurrentIndex(1));
        act(() => useFileStore.getState().setCurrentIndex(0));

        expect(asking.map((entry) => entry.source)).toEqual([HOLIDAY.identity, SUNSET.identity]);

        await asking[0]?.answer([{ family: "denoise" }]);
        expect(familiesOf(HOLIDAY.path)).toEqual(["denoise"]);
    });

    it("does not analyse a photograph again once it has answered", async () => {
        mount();
        await asking[0]?.answer([]);

        act(() => useFileStore.getState().setCurrentIndex(1));
        act(() => useFileStore.getState().setCurrentIndex(0));

        expect(asking.filter((entry) => entry.source === HOLIDAY.identity)).toHaveLength(1);
    });

    it("stops every analysis in flight when Autopilot is switched off", () => {
        mount();
        act(() => useFileStore.getState().setCurrentIndex(1));

        act(() => useEnhancementStore.getState().setAutopilot(false));

        expect(cancelled.mock.calls).toEqual([["suggest-1"], ["suggest-2"]]);
        expect(useAutopilotStore.getState().analysing.size).toBe(0);
    });

    it("analyses again when Autopilot is switched back on", () => {
        mount();
        act(() => useEnhancementStore.getState().setAutopilot(false));
        act(() => useEnhancementStore.getState().setAutopilot(true));

        expect(asking.map((entry) => entry.source)).toEqual([HOLIDAY.identity, HOLIDAY.identity]);
    });

    it("stops the analysis on a reframe and asks again at the new framing", async () => {
        frame(HOLIDAY);
        mount();

        frame(HOLIDAY, REFRAMED);

        expect(cancelled).toHaveBeenCalledExactlyOnceWith("suggest-1");
        expect(asking).toHaveLength(2);
        expect(asking[1]?.crop).toBe(REFRAMED);

        // Only the new one's suggestions are added, whichever order the two answer in.
        await asking[0]?.answer([{ family: "denoise" }]);
        await asking[1]?.answer([{ family: "sharpen" }]);

        expect(familiesOf(HOLIDAY.path)).toEqual(["sharpen"]);
    });

    it("stops the analysis on a reframe after a hand-added enhancement, and asks nothing again", async () => {
        frame(HOLIDAY);
        mount();

        const byHand = { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 } as const;
        act(() => useEnhancementStore.getState().addEnhancement(HOLIDAY.path, byHand));
        frame(HOLIDAY, REFRAMED);

        expect(cancelled).toHaveBeenCalledExactlyOnceWith("suggest-1");
        expect(asking).toHaveLength(1);
        expect(inFlight(HOLIDAY.path)).toBeUndefined();

        // The old answer lands nothing: not its suggestions, and not faces at the framing that is gone.
        await asking[0]?.answer([{ family: "denoise" }, { family: "face_recovery" }]);

        expect(detected).not.toHaveBeenCalled();
        expect(stackOf(HOLIDAY.path)).toEqual([byHand]);
    });

    it("does not retry a failure until the photograph becomes current again", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        mount();

        await asking[0]?.fail({ kind: "notReady" });
        expect(asking).toHaveLength(1);

        act(() => useFileStore.getState().setCurrentIndex(1));
        act(() => useFileStore.getState().setCurrentIndex(0));

        expect(asking.map((entry) => entry.source)).toEqual([HOLIDAY.identity, SUNSET.identity, HOLIDAY.identity]);
    });

    it("does not retry a failure until Autopilot is switched on again", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        mount();

        await asking[0]?.fail({ kind: "notReady" });
        act(() => useEnhancementStore.getState().setAutopilot(false));
        act(() => useEnhancementStore.getState().setAutopilot(true));

        expect(asking).toHaveLength(2);
    });

    it("never analyses a file with no identity", () => {
        resetFileStore();
        openFiles({ path: "/Users/someone/Pictures/unreadable.png", extension: "png" });

        mount();

        expect(asked).not.toHaveBeenCalled();
    });
});
