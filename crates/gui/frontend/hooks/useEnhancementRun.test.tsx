import { act, waitFor } from "@testing-library/react";
import { toast as sonner } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
// Initialised here, not by `AppProviders`: the run's notice is composed through a `t` held in a ref
// and raised outside the tree, so a toast would otherwise read back as its own key.
import "@/i18n";
import type { CropInfo } from "@/ipc/crop";
import type { Enhancement, Operation, RunProgress } from "@/ipc/enhance";
import { cancelEnhance, enhance, onEnhanceProgress } from "@/ipc/enhance";
import { detectFaces, type Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFacesStore } from "@/stores/faces";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";
import { FRAMING, HOLIDAY, openFiles, render, resetFileStore, SUNSET } from "@/test/support";
import { type EnhancementRun, useEnhancementRun } from "./useEnhancementRun";

// The seams themselves are pinned by `ipc/enhance.test.ts` and `ipc/faces.test.ts` against the wire;
// here they are mocked so this file is about what the hook asks for and what it does with the answers.
vi.mock("@/ipc/enhance", () => ({
    enhance: vi.fn(),
    cancelEnhance: vi.fn(() => Promise.resolve()),
    onEnhanceProgress: vi.fn(() => Promise.resolve(() => {})),
    // Not this hook's, but the enhancement store's: closing a photograph tells every owner to forget
    // it, and that one hands the result it was holding back to Rust.
    releaseEnhanced: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/ipc/faces", () => ({ detectFaces: vi.fn() }));

const asked = enhance as unknown as Mock;
const stopped = cancelEnhance as unknown as Mock;
const subscribed = onEnhanceProgress as unknown as Mock;
const detected = detectFaces as unknown as Mock;

/** One run the test settles by hand, in the shape `enhance` answers with. */
type Pending = { run: string; settle: (outcome: Enhancement) => void; reject: (error: unknown) => void };

const pending: Pending[] = [];

/** Delivers a report to whatever the hook subscribed with. */
const deliver = (report: RunProgress) => {
    for (const [handler] of subscribed.mock.calls) act(() => (handler as (r: RunProgress) => void)(report));
};

const upscale = (scale: number): Operation => ({ family: "upscale", codename: "kyoto", precision: "fp32", scale });

/** A face recovery as the add menu creates one: a model, and no faces until the run path puts them in. */
const recovery = (): Operation => ({ family: "face_recovery", codename: "athens", precision: "fp32", faces: [] });

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
});

/** One detection the test settles by hand, in the shape `detectFaces` answers with. */
type Detecting = { run: string; found: (faces: Face[]) => void; fail: (error: unknown) => void };

const detecting: Detecting[] = [];

const progress = (run: string, extra: Partial<RunProgress> = {}): RunProgress => ({
    run,
    operation: "Kyoto 2x (FP32)",
    family: "upscale",
    stage: "running",
    chainFraction: 0.4,
    ...extra,
});

/** What the hook answers, kept current by a probe component mounted the way the canvas mounts it. */
let answer: EnhancementRun;

const Probe = ({ file = HOLIDAY, crop }: { file?: typeof HOLIDAY; crop?: CropInfo }) => {
    answer = useEnhancementRun(file, crop);

    return null;
};

const mount = (file = HOLIDAY) => render(<Probe file={file} />);

/** A second framing, so a change of crop is a change of value rather than of one of its fields. */
const REFRAMED: CropInfo = { ...FRAMING, left: FRAMING.left + 200 };

/** Puts a stack on one image, which is what starts a run. */
const stack = (path: string, ...operations: Operation[]) =>
    act(() => useEnhancementStore.setState({ enhancements: new Map([[path, operations]]) }));

beforeEach(() => {
    vi.clearAllMocks();
    pending.length = 0;
    detecting.length = 0;
    sonner.dismiss();

    let minted = 0;
    asked.mockImplementation(() => {
        const run = `run-${++minted}`;
        const entry: Pending = { run, settle: () => {}, reject: () => {} };

        entry.settle = () => {};
        const done = new Promise<Enhancement>((resolve, reject) => {
            entry.settle = (outcome) => act(() => resolve(outcome));
            entry.reject = (error) => act(() => reject(error));
        });
        pending.push(entry);

        return { run, done };
    });

    let detections = 0;
    detected.mockImplementation(() => {
        const run = `detect-${++detections}`;
        const entry: Detecting = { run, found: () => {}, fail: () => {} };

        const done = new Promise<Face[]>((resolve, reject) => {
            entry.found = (faces) => act(() => resolve(faces));
            entry.fail = (error) => act(() => reject(error));
        });
        detecting.push(entry);

        return { run, done };
    });

    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    useFacesStore.setState(useFacesStore.getInitialState(), true);
    useSettingsStore.setState({ processor: "auto" });

    // Both photographs are open, as they are wherever this hook is given one: the canvas hands it a
    // record out of the file store, and what a detection answers is held only while that record is
    // still there.
    resetFileStore();
    openFiles(HOLIDAY, SUNSET);
});

describe("running the current image's enhancements", () => {
    it("asks for nothing while the image has none", () => {
        mount();

        // The seam accepts an empty chain and answers with the source, but that is a round trip for
        // an answer the window holds: the pane is pointed at the source directly.
        expect(asked).not.toHaveBeenCalled();
        expect(answer.enhanced).toBeUndefined();
    });

    it("runs the stack over the image and answers what it produced", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        expect(asked).toHaveBeenCalledWith(HOLIDAY.identity, [upscale(2)], "auto", undefined);

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });

        await waitFor(() =>
            expect(answer.enhanced).toEqual({ identity: "abcdef0123456789", width: 6000, height: 4000 }),
        );
    });

    it("runs again when the stack changes, and stops the run it replaces", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        stack(HOLIDAY.path, upscale(4));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(stopped).toHaveBeenCalledWith("run-1");
        expect(asked).toHaveBeenLastCalledWith(HOLIDAY.identity, [upscale(4)], "auto", undefined);
    });

    it("runs again when the processor changes", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        act(() => useSettingsStore.setState({ processor: "cpu" }));

        await waitFor(() => expect(asked).toHaveBeenLastCalledWith(HOLIDAY.identity, [upscale(2)], "cpu", undefined));
    });

    it("runs again when the framing changes, and stops the run it replaces", async () => {
        stack(HOLIDAY.path, upscale(2));
        const view = mount();

        view.rerender(<Probe file={HOLIDAY} crop={FRAMING} />);

        // The same rule the stack, the image and the processor already follow: the run in flight is
        // producing pixels for a framing nobody is looking at any more.
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(stopped).toHaveBeenCalledWith("run-1");
        expect(asked).toHaveBeenLastCalledWith(HOLIDAY.identity, [upscale(2)], "auto", FRAMING);
    });

    it("lets go of the result the previous framing produced", async () => {
        stack(HOLIDAY.path, upscale(2));
        const view = render(<Probe file={HOLIDAY} crop={FRAMING} />);

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 2400, height: 3200 });
        await waitFor(() => expect(answer.enhanced).toBeDefined());

        view.rerender(<Probe file={HOLIDAY} crop={REFRAMED} />);

        // The image-change behaviour rather than the enhancement-change behaviour, and a deliberate
        // divergence from the reference, which keeps the previous result on screen. The canvas sizes
        // its box from the framing's own dimensions, so a result made from other pixels would be
        // drawn to the wrong proportions until the re-run landed. D10.
        await waitFor(() => expect(answer.enhanced).toBeUndefined());

        // And the pixels themselves go: asking for a run is what displaces whatever the backend was
        // holding, enforced there rather than trusted to this side - see `ipc/enhance.ts`.
        expect(asked).toHaveBeenCalledTimes(2);
    });

    it("keeps the result when an enhancement changes, where it drops it for a framing", async () => {
        stack(HOLIDAY.path, upscale(2));
        render(<Probe file={HOLIDAY} crop={FRAMING} />);

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 2400, height: 3200 });
        await waitFor(() => expect(answer.enhanced).toBeDefined());

        stack(HOLIDAY.path, upscale(4));

        // The other half of D10, and why the two are not one rule: an enhancement does not change the
        // shape of what is drawn, so the previous result stays on screen until a newer one lands -
        // which is what stops the scale field flashing the source between keystrokes.
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(answer.enhanced).toEqual({ identity: "abcdef0123456789", width: 2400, height: 3200 });
    });

    it("asks for no run when the framing of an image with no enhancements changes", async () => {
        const view = mount();

        view.rerender(<Probe file={HOLIDAY} crop={FRAMING} />);

        // An empty stack asks for nothing whatever else changes: the seam would answer with the
        // source, and that is a round trip for an answer the window already holds. The canvas draws
        // the newly framed source, which it gets from the rendition URL rather than from a run.
        await act(async () => {});
        expect(asked).not.toHaveBeenCalled();
        expect(answer.enhanced).toBeUndefined();
    });

    it("stops a run nothing is waiting for any more", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        // The last enhancement removed: no new run displaces this one, so the explicit stop is the
        // only thing that ends it.
        stack(HOLIDAY.path);

        await waitFor(() => expect(stopped).toHaveBeenCalledWith("run-1"));
        expect(asked).toHaveBeenCalledTimes(1);
        expect(answer.enhanced).toBeUndefined();
    });

    it("drops the result when the image changes", async () => {
        stack(HOLIDAY.path, upscale(2));
        const view = mount();

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });
        await waitFor(() => expect(answer.enhanced).toBeDefined());

        view.rerender(<Probe file={SUNSET} />);

        // A result belongs to the photograph it was made from; drawing it over another one is the
        // one thing the second pane is not for.
        await waitFor(() => expect(answer.enhanced).toBeUndefined());
    });

    it("writes nothing for a run that was stopped", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        pending[0]?.settle({ outcome: "stopped" });

        // A displaced run answers `stopped` whatever it returned, so the view goes on showing what
        // it was showing rather than a partly-enhanced image.
        await waitFor(() => expect(answer.enhanced).toBeUndefined());
    });

    it("holds the last result while the run that replaces it works", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });
        await waitFor(() => expect(answer.enhanced).toBeDefined());

        stack(HOLIDAY.path, upscale(4));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        pending[0]?.settle({ outcome: "stopped" });

        // Typing in the scale field re-runs on every keystroke; flashing the source between them is
        // what holding the last result avoids.
        await waitFor(() => expect(answer.enhanced?.identity).toBe("abcdef0123456789"));
    });
});

describe("what the canvas is told about a run", () => {
    it("says a run is in flight before it has reported anything", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        // Which is what the indicator is drawn on: a run loads a model before it reports, and an
        // indicator that waited for the first report appeared long after the enhancement was added.
        await waitFor(() => expect(answer.running).toBe(true));
        expect(answer.report).toBeUndefined();
    });

    it("stops saying so once the run has ended", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        await waitFor(() => expect(answer.running).toBe(true));

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });

        await waitFor(() => expect(answer.running).toBe(false));
    });

    it("says nothing is in flight once the last enhancement is removed", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        await waitFor(() => expect(answer.running).toBe(true));

        stack(HOLIDAY.path);

        await waitFor(() => expect(answer.running).toBe(false));
    });

    it("keeps the reports naming its own run", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        deliver(progress("run-1"));
        await waitFor(() => expect(answer.report?.chainFraction).toBe(0.4));
    });

    it("discards a report from a run it has abandoned", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        stack(HOLIDAY.path, upscale(4));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        // A displaced run goes on reporting until it notices it has been stopped, which is what
        // makes those reports discardable rather than confusing.
        deliver(progress("run-1", { chainFraction: 0.9 }));

        expect(answer.report).toBeUndefined();
    });

    it("stops reporting once the run has ended", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        deliver(progress("run-1"));
        await waitFor(() => expect(answer.report).toBeDefined());

        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });

        await waitFor(() => expect(answer.report).toBeUndefined());
    });

    it("reports nothing for a run whose every operation was already known", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        // Such a chain produces no report at all - each operation was neither fetched nor run - so
        // the bar is drawn empty and generically labelled for as long as the chain takes, and never
        // says which enhancement it is on.
        pending[0]?.settle({ outcome: "enhanced", identity: "abcdef0123456789", width: 6000, height: 4000 });

        await waitFor(() => expect(answer.enhanced).toBeDefined());
        expect(answer.report).toBeUndefined();
    });
});

describe("a run that does not finish", () => {
    const toast = () => document.querySelector("[data-sonner-toast]");

    it("tells the user when a run fails", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        pending[0]?.reject({ kind: "enhance", message: "no execution provider could be built" });

        await waitFor(() => expect(toast()).toHaveTextContent("Failed to enhance image"));
        expect(answer.enhanced).toBeUndefined();
    });

    it("says nothing about a run that was stopped", async () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        pending[0]?.settle({ outcome: "stopped" });

        // The user asked for it to stop. Reporting that as a failure would tell them their
        // enhancement broke when they had merely changed their mind.
        await waitFor(() => expect(answer.enhanced).toBeUndefined());
        expect(toast()).toBeNull();
    });
});

describe("a chain that restores faces", () => {
    const toast = () => document.querySelector("[data-sonner-toast]");

    const faces = () => useFacesStore.getState().faces;

    it("detects nothing for a photograph whose enhancements do not restore faces", () => {
        // Detection costs a model in memory and, on a first run, transferring one. A window that
        // detected in every photograph it opened would pay that for photographs nobody is going to
        // restore a face in.
        stack(HOLIDAY.path, upscale(2));
        mount();

        expect(detected).not.toHaveBeenCalled();
        expect(asked).toHaveBeenCalledTimes(1);
    });

    it("finds the faces before running anything, and runs nothing until they land", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        expect(detected).toHaveBeenCalledWith(HOLIDAY.identity, "auto", undefined);

        // The whole of what this arrangement is for: a run started before the faces arrive would
        // restore nothing, would be stored as the result for an empty selection, and would have to be
        // run again the moment they landed.
        expect(asked).not.toHaveBeenCalled();

        detecting[0]?.found([face(0), face(20)]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        expect(asked).toHaveBeenCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0), face(20)] }],
            "auto",
            undefined,
        );
    });

    it("leaves the stack itself carrying no faces", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // The stack is the user's choice and the faces are a property of the pixels: they are put in
        // on the way to the run and nowhere else, so a framing change rewrites no photograph's stack.
        expect(useEnhancementStore.getState().enhancements.get(HOLIDAY.path)).toEqual([recovery()]);
    });

    it("puts the faces only into the operations that restore them", async () => {
        stack(HOLIDAY.path, recovery(), upscale(2));
        mount();

        detecting[0]?.found([face(0)]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        expect(asked).toHaveBeenCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0)] }, upscale(2)],
            "auto",
            undefined,
        );
    });

    it("detects nothing again once the faces for the framing in force are known", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // Adding a second enhancement and changing the processor are both re-runs, and neither is a
        // question about the pixels the detector already answered.
        stack(HOLIDAY.path, recovery(), upscale(2));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        act(() => useSettingsStore.setState({ processor: "cpu" }));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(3));

        expect(detected).toHaveBeenCalledTimes(1);
    });

    it("detects again when the framing changes", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        detecting[0]?.found([face(0), face(20)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // A flip, a turn or a cut moves every face and can add or remove one, so the previous answer
        // describes a photograph nobody is looking at any more.
        view.rerender(<Probe crop={FRAMING} />);

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(2));
        expect(detected).toHaveBeenLastCalledWith(HOLIDAY.identity, "auto", FRAMING);

        detecting[1]?.found([face(0)]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0)] }],
            "auto",
            FRAMING,
        );
    });

    it("holds one answer per photograph, so a framing change discards the previous one", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        detecting[0]?.found([face(0), face(20)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        view.rerender(<Probe crop={FRAMING} />);
        await waitFor(() => expect(detected).toHaveBeenCalledTimes(2));
        detecting[1]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        // One entry, replaced - not a cache of every framing the user has passed through. This store
        // is a cache of the run path's own step; the cache of framings is `opai`'s run store, which is
        // bounded, shared and outlives the process. A second unbounded one here would need an
        // eviction policy of its own for nothing. See design.md D9.
        expect(faces().size).toBe(1);
        expect(faces().get(HOLIDAY.identity ?? "")).toEqual({ crop: FRAMING, faces: [face(0)] });
    });

    it("asks again for a framing returned to, and answers what was found the first time", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        detecting[0]?.found([face(0), face(20)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        view.rerender(<Probe crop={FRAMING} />);
        await waitFor(() => expect(detected).toHaveBeenCalledTimes(2));
        detecting[1]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        // Back to the framing detected at first. A third request is made and **no detection model is
        // run for it**: a framing returned to is one identity returned to, so `opai`'s run store
        // answers it before anything is transferred or opened. That is where the saving is, and it is
        // why this side keeps no per-framing cache of its own. See design.md D3.
        view.rerender(<Probe />);

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(3));
        expect(detected).toHaveBeenLastCalledWith(HOLIDAY.identity, "auto", undefined);

        detecting[2]?.found([face(0), face(20)]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(3));
        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0), face(20)] }],
            "auto",
            undefined,
        );
    });

    it("reports a detection's progress under the run it named", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        const report = progress("detect-1", { family: "detection", stage: "installing", installFraction: 0.4 });
        deliver(report);

        // The indicator is on screen from the moment the detection is asked for, and the transfer of
        // the detector is what it is drawing: the first thing a user sees of a face recovery.
        await waitFor(() => expect(answer.report).toEqual(report));
        expect(answer.running).toBe(true);
    });

    it("says nothing about a photograph with nobody in it, and runs the chain", async () => {
        stack(HOLIDAY.path, recovery(), upscale(2));
        mount();

        detecting[0]?.found([]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        expect(asked).toHaveBeenCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [] }, upscale(2)],
            "auto",
            undefined,
        );
        expect(toast()).toBeNull();
    });

    it("tells the user when a detection fails, and still runs the chain", async () => {
        stack(HOLIDAY.path, recovery(), upscale(2));
        mount();

        detecting[0]?.fail({ kind: "detect", message: "no execution provider could be built" });

        await waitFor(() => expect(toast()).toHaveTextContent("Failed to detect faces"));

        // A failed detection means the faces are not restored. It must not also mean that the upscale
        // the user asked for in the same list does not happen.
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        expect(asked).toHaveBeenCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [] }, upscale(2)],
            "auto",
            undefined,
        );
    });

    it("records a failed detection as no faces rather than leaving the question unanswered", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        detecting[0]?.fail({ kind: "detect", message: "no execution provider could be built" });
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // Otherwise the run would be held forever waiting for an answer that is never coming, and the
        // next re-run would detect again and fail again.
        expect(faces().get(HOLIDAY.identity ?? "")).toEqual({ faces: [] });

        stack(HOLIDAY.path, recovery(), upscale(2));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(detected).toHaveBeenCalledTimes(1);
    });

    it("asks nothing further while the detection that will answer it is still in flight", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        // The store cannot say a detection is in flight - its entry appears only once the answer
        // lands - so without a guard each of these would ask the same question again, and two
        // detectors would run over the same pixels at once.
        stack(HOLIDAY.path, recovery(), upscale(2));
        act(() => useSettingsStore.setState({ processor: "cpu" }));

        expect(detected).toHaveBeenCalledTimes(1);
        expect(asked).not.toHaveBeenCalled();

        detecting[0]?.found([face(0)]);

        // And the answer serves the stack as it stands now, not as it stood when it was asked for.
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        expect(asked).toHaveBeenCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0)] }, upscale(2)],
            "cpu",
            undefined,
        );
    });

    it("goes on drawing the detection in flight across a change that asks nothing", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));
        stack(HOLIDAY.path, recovery(), upscale(2));

        // The pass that asked nothing still has to name what it is waiting for, or the chip would go
        // out for the rest of a transfer the window is still sitting through.
        deliver(progress("detect-1", { family: "detection" }));

        expect(answer.running).toBe(true);
        expect(answer.report?.run).toBe("detect-1");
    });

    it("holds nothing for a photograph that was closed while its detection was in flight", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        // Closing has already told every owner to forget this photograph. An answer landing after
        // that would put an entry back that nothing empties, since the close it belongs to is over.
        act(() => useFileStore.getState().closeFile(HOLIDAY.path));
        view.rerender(<Probe file={SUNSET} />);

        detecting[0]?.found([face(0), face(20)]);
        await act(async () => {});

        expect(faces().size).toBe(0);
    });

    it("holds nothing for a photograph closed while a detection that then failed was in flight", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        act(() => useFileStore.getState().closeFile(HOLIDAY.path));
        view.rerender(<Probe file={SUNSET} />);

        detecting[0]?.fail({ kind: "detect", message: "no execution provider could be built" });

        // The notice is still raised: what it reports is that a detection this window asked for
        // failed, which is true whether or not the photograph is still open.
        await waitFor(() => expect(toast()).toHaveTextContent("Failed to detect faces"));
        expect(faces().has(HOLIDAY.identity ?? "")).toBe(false);
    });

    it("holds what a detection answers for a photograph the window has only moved off", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        // Moving to another photograph is not closing this one, and a late answer for it is a warm
        // cache: it is read by nobody until that photograph is current again.
        view.rerender(<Probe file={SUNSET} />);
        detecting[0]?.found([face(0)]);

        await waitFor(() => expect(faces().get(HOLIDAY.identity ?? "")).toEqual({ faces: [face(0)] }));
    });

    it("keeps one photograph's faces out of another's run", async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        detecting[0]?.found([face(0), face(20)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        act(() =>
            useEnhancementStore.setState({
                enhancements: new Map([
                    [HOLIDAY.path, [recovery()]],
                    [SUNSET.path, [recovery()]],
                ]),
            }),
        );
        view.rerender(<Probe file={SUNSET} />);

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(2));
        expect(detected).toHaveBeenLastCalledWith(SUNSET.identity, "auto", undefined);
    });
});

describe("which of a photograph's faces a chain restores", () => {
    /** Applies a choice, as the Select faces dialog's Apply does: one write however many boxes were clicked. */
    const skip = (...faces: Face[]) =>
        act(() => useFacesStore.getState().setSkippedFaces(HOLIDAY.identity ?? "", new Set(faces.map(faceKey))));

    /** A chain over a photograph whose two faces have already been found. */
    const detectedTwo = async () => {
        stack(HOLIDAY.path, recovery());
        const view = mount();

        detecting[0]?.found([face(0), face(20)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        return view;
    };

    it("carries every face found while none are skipped", async () => {
        await detectedTwo();

        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(0), face(20)] }],
            "auto",
            undefined,
        );
    });

    it("carries only the chosen faces once a choice is applied", async () => {
        await detectedTwo();

        skip(face(0));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(20)] }],
            "auto",
            undefined,
        );
    });

    it("starts exactly one run for an applied choice, whatever it changed", async () => {
        // The dialog writes once, on Apply, however many boxes were clicked - so this is one run and
        // not one per face. It stops the run it replaces, as every other change to a chain does.
        await detectedTwo();

        skip(face(0), face(20));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(stopped).toHaveBeenCalledWith("run-1");
    });

    it("starts no run for a choice that changes nothing", async () => {
        // A write is what re-runs the chain, so "applying a choice that changes nothing starts no run"
        // rests on that apply writing nothing - which is `useFaceSelection`'s guard, pinned in its own
        // file. What this pins is the other half: an unchanged selection reaches nothing here.
        await detectedTwo();

        const committed = useFacesStore.getState().skipped;

        act(() => {
            useFacesStore.setState({ skipped: committed });
        });

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
    });

    it("runs the chain with no faces at all when every one of them is skipped", async () => {
        // A face-recovery operation carrying no faces is a request rather than a failure: nothing is
        // restored and every other operation in the chain is still applied.
        stack(HOLIDAY.path, recovery(), upscale(2));
        mount();

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        skip(face(0));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [] }, upscale(2)],
            "auto",
            undefined,
        );
    });

    it("re-runs nothing on a render that wrote no choice", async () => {
        // The filtered array is built inside the effect for exactly this: computing it in the render
        // body and listing it would hand the effect a fresh array - and a new run - every render.
        const view = await detectedTwo();

        skip(face(0));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        view.rerender(<Probe file={HOLIDAY} />);
        view.rerender(<Probe file={HOLIDAY} />);

        expect(asked).toHaveBeenCalledTimes(2);
    });

    it("detects nothing again when a choice is applied", async () => {
        // A choice is not a question about the pixels: the faces for this framing are already known.
        await detectedTwo();

        skip(face(20));

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));
        expect(detected).toHaveBeenCalledTimes(1);
    });

    it("leaves the chain alone for a choice recorded against another photograph", async () => {
        await detectedTwo();

        act(() => useFacesStore.getState().setSkippedFaces(SUNSET.identity ?? "", new Set([faceKey(face(0))])));

        expect(asked).toHaveBeenCalledTimes(1);
    });

    it("carries every face again when a framing change re-detects them", async () => {
        // Nothing here resets the choice: the faces in the new framing are at new coordinates, so the
        // keys skipped at the old one match none of them.
        const view = await detectedTwo();

        skip(face(0));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        view.rerender(<Probe file={HOLIDAY} crop={FRAMING} />);
        await waitFor(() => expect(detected).toHaveBeenCalledTimes(2));

        detecting[1]?.found([face(40), face(60)]);

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(3));
        expect(asked).toHaveBeenLastCalledWith(
            HOLIDAY.identity,
            [{ ...recovery(), faces: [face(40), face(60)] }],
            "auto",
            FRAMING,
        );
    });
});

/*
 * What the bar draws while a face recovery works.
 *
 * Underneath it there are two runs - the detection and then the chain - each reporting its own
 * `0..1` and each landing on exactly 1. Drawn as they arrived that filled the bar, emptied it and
 * filled it again, which reads as the enhancement having been applied twice. These pin the one
 * figure that spans both.
 */
describe("one bar across a detection and the chain it was run for", () => {
    /** Where the bar is, as a whole percentage - which is all it is ever drawn as. */
    const at = () => Math.round(answer.fraction * 100);

    /** A detection reporting `fraction` of its own run. */
    const detecting1 = (fraction: number) =>
        deliver(progress("detect-1", { family: "detection", operation: "New York (FP32)", chainFraction: fraction }));

    /** The chain reporting `fraction` of its own run. */
    const running = (run: string, fraction: number) =>
        deliver(progress(run, { family: "face_recovery", chainFraction: fraction }));

    it("gives the detection the head of the bar and the chain what it leaves", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        // A fifth, which is the reference's own `progressAfterDetect`.
        detecting1(0);
        expect(at()).toBe(0);
        detecting1(0.5);
        expect(at()).toBe(10);
        detecting1(1);
        expect(at()).toBe(20);

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        running("run-1", 0);
        expect(at()).toBe(20);
        running("run-1", 0.5);
        expect(at()).toBe(60);
        running("run-1", 1);
        expect(at()).toBe(100);
    });

    it("never takes the bar backwards, and fills it exactly once", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        const drawn: number[] = [];
        const sample = () => drawn.push(at());

        for (const fraction of [0, 0.25, 0.5, 0.75, 1]) {
            detecting1(fraction);
            sample();
        }

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
        // Where the bar sits through the seconds the chain spends loading its model, which is the
        // interval that used to show it empty again.
        sample();

        for (const fraction of [0, 0.3, 0.6, 1]) {
            running("run-1", fraction);
            sample();
        }

        expect(drawn.every((position, index) => index === 0 || position >= (drawn[index - 1] ?? 0))).toBe(true);
        expect(drawn.filter((position) => position === 100)).toHaveLength(1);
        expect(drawn.at(-1)).toBe(100);
    });

    it("goes on naming the enhancement while the chain loads its model", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));
        detecting1(1);

        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // The chain reports nothing while it loads, and a report cleared here left the chip reading
        // the generic label for those seconds - the enhancement appearing to start over.
        expect(answer.report?.family).toBe("detection");
        expect(at()).toBe(20);
    });

    it("gives a chain that needed no detection the whole bar", () => {
        stack(HOLIDAY.path, upscale(2));
        mount();

        deliver(progress("run-1", { chainFraction: 0.5 }));

        expect(at()).toBe(50);
    });

    it("gives the bar back in full to a re-run over faces already known", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));
        detecting[0]?.found([face(0)]);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        // A second run over the same faces - here a model changed, but a scale or the processor is
        // the same thing. Nothing is detected, so nothing is owed the head of the bar.
        stack(HOLIDAY.path, { ...recovery(), codename: "santorini" });
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(2));

        expect(detected).toHaveBeenCalledTimes(1);
        running("run-2", 0.5);
        expect(at()).toBe(50);
    });

    it("runs a stack Autopilot wrote at once, over the faces it recorded, with the whole bar", async () => {
        // Autopilot's handover: its follow-up detection recorded the faces at the crop in force, and then its
        // batch wrote a stack carrying face recovery. The run has nothing left to detect.
        render(<Probe crop={FRAMING} />);
        act(() => useFacesStore.getState().setFaces(HOLIDAY.identity ?? "", FRAMING, [face(0)]));
        stack(HOLIDAY.path, recovery());

        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        expect(detected).not.toHaveBeenCalled();
        expect(asked).toHaveBeenCalledWith(HOLIDAY.identity, [{ ...recovery(), faces: [face(0)] }], "auto", FRAMING);

        running("run-1", 0.5);
        expect(at()).toBe(50);
    });

    it("does not hand another photograph's chain the tail a detection left", async () => {
        // The detection was for HOLIDAY. SUNSET's upscale follows it in time and has nothing to do
        // with it, so it owns the whole bar - a bare "a detection happened" would start it at a
        // fifth.
        act(() =>
            useEnhancementStore.setState({
                enhancements: new Map([
                    [HOLIDAY.path, [recovery()]],
                    [SUNSET.path, [upscale(2)]],
                ]),
            }),
        );
        const view = mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        view.rerender(<Probe file={SUNSET} />);
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        running("run-1", 0.5);
        expect(at()).toBe(50);
    });

    it("leaves a running chain alone when a detection lands for a stack that no longer needs faces", async () => {
        stack(HOLIDAY.path, recovery());
        mount();

        await waitFor(() => expect(detected).toHaveBeenCalledTimes(1));

        // The face recovery is taken out while its detection is still in flight, so the chain that
        // replaces it needs no faces and runs at once.
        stack(HOLIDAY.path, upscale(2));
        await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));

        detecting[0]?.found([face(0)]);
        await act(async () => {});

        // The answer is still recorded - it is a warm cache for the framing it was found at - but it
        // is not a question this stack asked, so the run in flight is not cancelled and re-asked.
        expect(useFacesStore.getState().faces.get(HOLIDAY.identity ?? "")).toEqual({ faces: [face(0)] });
        expect(asked).toHaveBeenCalledTimes(1);
        expect(stopped).not.toHaveBeenCalledWith("run-1");
    });
});
