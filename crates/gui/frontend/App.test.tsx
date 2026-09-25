import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import type { SetupEvent, SupportedProviders } from "@/ipc/setup";
import { track } from "@/lib/faro";
import { useSettingsStore } from "@/stores/settings";
import {
    CATALOGUE,
    HOLIDAY,
    openFiles,
    PLAN,
    PROVIDERS,
    REJECTION,
    render,
    resetFileStore,
    resetSettingsStore,
    resetSetupStore,
} from "@/test/support";
import App from "./App";

// The three IPC wrappers the shell reaches through, all pinned by their own tests. `@/ipc/os` has to
// be mocked rather than merely stubbed: the real `platform()` reads a global the plugin's init script
// installs into the webview, which does not exist in jsdom. `@/ipc/setup` for the same kind of
// reason - `new Channel()` reads another of those globals - and because these tests are about what
// the shell does with the reports, not about how they cross the boundary.
vi.mock("@/ipc/app", () => ({
    appVersion: vi.fn(() => Promise.resolve("26.9.0")),
    isOutdated: vi.fn(() => Promise.resolve(false)),
}));
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));
vi.mock("@/ipc/setup", () => ({ initialize: vi.fn(), quit: vi.fn() }));

// The window's drag-drop channel, for the same reason as the three above: `getCurrentWebview` reads a
// global the webview's init script installs, and jsdom has none. What the listener does with a drop is
// `hooks/useDroppedImages.test.tsx`'s subject; here it only has to be registrable.
vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));

// And `convertFileSrc`, which reads another of them - the canvas builds a URL with it the moment an
// image is open. What it answers is asserted against in `PreviewImage.test.tsx`. `invoke` is beside
// it because the sidebar's add menu asks for the catalogue as soon as the shell mounts; what that
// menu does with the answer is `AddEnhancement.test.tsx`'s subject. An image opened with Autopilot on is
// analysed, and the analysis answers that it needs nothing: what the window does with an answer is
// `hooks/useAutopilot.test.tsx`'s subject.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
    invoke: vi.fn((command: string) => Promise.resolve(command === "suggest" ? [] : CATALOGUE)),
}));

// And the progress event, which `listen` reads a third of those globals for: a mounted canvas
// subscribes to it for as long as a file is open. What the listener does with a report is
// `hooks/useEnhancementRun.test.tsx`'s subject; here it only has to be registrable.
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const { initialize } = await import("@/ipc/setup");

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));
const tracked = track as unknown as Mock;
const readies = () => tracked.mock.calls.filter(([event]) => event === "app_ready");

/** Drives the mounted application's own initialization: its reports, and how it ends. */
let report: (event: SetupEvent) => void;
let finish: (providers?: SupportedProviders) => void;
let fail: (reason: unknown) => void;

const setupDialog = () => screen.queryByRole("dialog");
const errorDialog = () => screen.queryByRole("alertdialog");

beforeEach(() => {
    resetSetupStore();
    resetFileStore();
    localStorage.clear();
    resetSettingsStore();

    // Re-established every test because `restoreMocks` is on in vite.config.ts. The promise is left
    // pending so each test decides how this launch ends.
    (initialize as Mock).mockImplementation((onEvent: (event: SetupEvent) => void) => {
        report = onEvent;

        // Resolving with the provider report rather than with nothing, because that is what the real
        // command answers: the fact rides on this promise, and a mock that resolved empty would let
        // the shell drop it without a test noticing.
        return new Promise<SupportedProviders>((resolve, reject) => {
            finish = (providers = PROVIDERS) => resolve(providers);

            fail = (reason: unknown) => {
                // Rust logs the failure as well, and these tests are about what is left on screen
                // rather than about that. Silenced by the call that provokes it rather than for the
                // whole file, so the tests that never fail keep React's own channel - `act`
                // warnings and error-boundary reports come through it, and this is the file that
                // mounts the entire shell. `restoreMocks` puts it back between tests.
                vi.spyOn(console, "error").mockImplementation(() => {});

                reject(reason);
            };
        });
    });
});

describe("App", () => {
    it("shows all four regions at once", async () => {
        const { container } = render(<App />);

        // Each region by something only it renders, rather than by a test id: the navbar's version,
        // the canvas's invitation, the drawer's Add images and the sidebar's empty preview.
        expect(await screen.findByText("v26.9.0")).toBeInTheDocument();
        expect(container.querySelector("p")).toContainHTML("Drag and drop images<br>to start editing them");
        expect(screen.getByRole("button", { name: "Add images" })).toBeInTheDocument();
        expect(screen.getByText("No preview available")).toBeInTheDocument();
    });

    it("disables every control whose effect is on an image", () => {
        const { container } = render(<App />);

        expect(container.querySelector("[data-slot='drawer-fold']")).toBeDisabled();
        expect(screen.getByRole("checkbox")).toBeDisabled();

        for (const mode of ["Full", "Side by Side", "Split"]) {
            expect(screen.getByRole("radio", { name: mode })).toBeDisabled();
        }

        expect(screen.getByRole("slider")).toHaveAttribute("data-disabled");
        expect(screen.getByRole("button", { name: "Add enhancement" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "Export image" })).toBeDisabled();
    });

    it("draws the image and turns on the comparison group when one is opened", () => {
        render(<App />);

        act(() => openFiles(HOLIDAY));

        // The frame leaves its empty state in one place: `hasFiles` is a store read, so the canvas
        // swaps the invitation for the photograph and the drawer's comparison group lights up
        // without either of them being told.
        expect(screen.queryByRole("button", { name: "Browse images" })).not.toBeInTheDocument();
        expect(screen.getAllByRole("img", { name: "Preview" })).toHaveLength(2);
        expect(screen.getByRole("radio", { name: "Side by Side" })).toBeEnabled();
    });

    it("makes the image controls live once an image is open, export included for the one it picks", () => {
        render(<App />);

        act(() => openFiles(HOLIDAY));

        // Stated from the shell, which is what a user sees: a batch opened into an empty window picks its
        // first file, so Export image is live along with Add enhancement.
        expect(screen.getByRole("slider", { name: "Zoom" })).not.toHaveAttribute("data-disabled");
        expect(screen.getByRole("button", { name: "Add enhancement" })).toBeEnabled();
        expect(screen.getByRole("button", { name: "Export image" })).toBeEnabled();
    });

    it("makes the drawer's own controls live once an image is open", () => {
        render(<App />);

        act(() => openFiles(HOLIDAY));

        expect(screen.getByRole("button", { name: "Show images" })).toBeEnabled();
        expect(screen.getByLabelText(/select all/i)).toBeEnabled();
    });

    it("keeps Autopilot live, because it is not an action on an image", () => {
        render(<App />);

        expect(screen.getByRole("switch", { name: "Autopilot" })).toBeEnabled();
    });

    it("starts initializing on mount, without being asked", () => {
        render(<App />);

        expect(initialize).toHaveBeenCalledTimes(1);
    });

    it("opens no dialog on the plan alone", () => {
        render(<App />);

        act(() => report(PLAN));

        // The plan is the list of what this machine needs, not a statement that any of it is being
        // fetched: every component ahead of the first transfer still has to say whether it had work.
        expect(setupDialog()).not.toBeInTheDocument();
    });

    it("opens the dialog on the first report of work", () => {
        render(<App />);

        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "ONNX Runtime", state: "downloading", fraction: 0.01 });
        });

        expect(setupDialog()).toBeInTheDocument();
        expect(screen.getByText("Downloading")).toBeInTheDocument();
    });

    it("opens no dialog at all when every component was already installed", () => {
        render(<App />);

        act(() => {
            report(PLAN);
            for (const row of PLAN.rows) {
                report({ kind: "progress", name: row.name, state: "installed", fraction: 1 });
            }
        });
        expect(setupDialog()).not.toBeInTheDocument();

        act(() => finish());
        expect(setupDialog()).not.toBeInTheDocument();
    });

    it("takes the dialog down when initialization succeeds", async () => {
        render(<App />);

        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "NVIDIA CUDA", state: "downloading", fraction: 0.5 });
        });
        expect(setupDialog()).toBeInTheDocument();

        // Awaited rather than wrapped in `act` alone: the resolution runs through a `.then`, so the
        // state update lands a microtask later.
        await act(async () => finish());

        expect(setupDialog()).not.toBeInTheDocument();
    });

    it("sends app_ready once initialization succeeds, with what this load runs with", async () => {
        useSettingsStore.getState().apply({ processor: "coreml", language: "sv", background: "dotted" });
        render(<App />);
        expect(readies()).toEqual([]);

        await act(async () => finish());

        expect(readies()).toEqual([
            ["app_ready", { processor: "coreml", language: "sv", background: "dotted", autopilot: true }],
        ]);
    });

    it("sends app_ready once per load, however many answers arrive", async () => {
        // A second mount, as StrictMode makes in development, asks and succeeds a second time.
        render(<App />);
        await act(async () => finish());
        render(<App />);
        await act(async () => finish());

        expect(readies()).toHaveLength(1);
    });

    it("sends no app_ready for a failed launch, and one for the retry that succeeds", async () => {
        render(<App />);
        await act(async () => fail(REJECTION));
        expect(readies()).toEqual([]);

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));
        await act(async () => finish());

        expect(readies()).toHaveLength(1);
    });

    it("replaces the setup dialog with the failure dialog when initialization fails", async () => {
        render(<App />);

        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "NVIDIA CUDA", state: "downloading", fraction: 0.09 });
        });
        expect(setupDialog()).toBeInTheDocument();

        await act(async () => fail(REJECTION));

        expect(setupDialog()).not.toBeInTheDocument();
        expect(errorDialog()).toBeInTheDocument();
        expect(screen.getByText("Couldn't finish setting up")).toBeInTheDocument();
        expect(screen.getByText(REJECTION.message)).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
    });

    it("puts the setup dialog back and runs the same command again when Try again is pressed", async () => {
        render(<App />);

        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "ONNX Runtime", state: "downloading", fraction: 0.4 });
        });
        await act(async () => fail(REJECTION));

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));

        // Back to the setup dialog in the same render the failure dialog left in, and the same
        // command invoked a second time - not a narrower repair of the component that stopped.
        expect(errorDialog()).not.toBeInTheDocument();
        expect(setupDialog()).toBeInTheDocument();
        expect(initialize).toHaveBeenCalledTimes(2);

        // The previous attempt's rows until the retry's own plan replaces them, which is what the
        // previous attempt actually found.
        expect(screen.getByText("ONNX Runtime")).toBeInTheDocument();
    });

    it("takes both dialogs down when a retry succeeds", async () => {
        render(<App />);

        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "ONNX Runtime", state: "downloading", fraction: 0.4 });
        });
        await act(async () => fail(REJECTION));

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));
        await act(async () => {
            report(PLAN);
            for (const row of PLAN.rows) {
                report({ kind: "progress", name: row.name, state: "installed", fraction: 1 });
            }
            finish();
        });

        expect(setupDialog()).not.toBeInTheDocument();
        expect(errorDialog()).not.toBeInTheDocument();
    });

    it("withholds Try again for a failure a second attempt cannot fix", async () => {
        render(<App />);

        await act(async () =>
            fail({
                kind: "initialize",
                failure: "unrecoverable",
                message: "no ONNX Runtime is published for solaris/sparc",
            }),
        );

        expect(errorDialog()).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
    });
});

describe("App, on a machine that supports TensorRT", () => {
    /** What an RTX card's launch reports: both NVIDIA providers, on top of the CPU. */
    const RTX: SupportedProviders = { cpu: true, coreml: false, cuda: true, tensorrt: true };

    const question = () => screen.queryByRole("dialog", { name: "TensorRT Detected" });

    /** A launch with a component to install, left with the setup dialog up. */
    const installing = () =>
        act(() => {
            report(PLAN);
            report({ kind: "progress", name: "NVIDIA TensorRT", state: "downloading", fraction: 0.5 });
        });

    it("asks once the setup dialog has gone away", async () => {
        render(<App />);

        installing();
        // Not while the setup dialog is up.
        expect(question()).not.toBeInTheDocument();

        await act(async () => finish(RTX));

        // The question is the only dialog on screen: the setup dialog went down as it came up.
        expect(screen.getAllByRole("dialog")).toEqual([question()]);
        expect(question()).toBeInTheDocument();
    });

    it("never asks a machine whose report does not offer TensorRT", async () => {
        render(<App />);

        await act(async () => finish({ ...RTX, tensorrt: false }));

        expect(question()).not.toBeInTheDocument();
    });

    it("does not ask a user who answered on an earlier launch", async () => {
        useSettingsStore.setState({ tensorrtAsked: true });
        render(<App />);

        await act(async () => finish(RTX));

        expect(question()).not.toBeInTheDocument();
    });

    it("does not ask on a launch whose setup failed", async () => {
        render(<App />);

        installing();
        await act(async () => fail(REJECTION));

        expect(errorDialog()).toBeInTheDocument();
        expect(question()).not.toBeInTheDocument();
    });

    it("asks after a retry that succeeds, as after a first attempt", async () => {
        render(<App />);

        installing();
        await act(async () => fail(REJECTION));

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));
        // The setup dialog is back, and the question is still not asked over it.
        expect(question()).not.toBeInTheDocument();

        await act(async () => finish(RTX));

        expect(errorDialog()).not.toBeInTheDocument();
        expect(question()).toBeInTheDocument();
    });

    it("goes away once answered, having recorded the answer", async () => {
        render(<App />);

        await act(async () => finish(RTX));
        fireEvent.click(screen.getByRole("button", { name: "No" }));

        expect(question()).not.toBeInTheDocument();
        expect(useSettingsStore.getState()).toMatchObject({ processor: "cuda", tensorrtAsked: true });
        expect(tracked).toHaveBeenCalledWith("tensorrt_prompt_answered", { accepted: false });
    });

    it("sends the answer when it is accepted", async () => {
        render(<App />);

        await act(async () => finish(RTX));
        fireEvent.click(screen.getByRole("button", { name: "Yes" }));

        expect(tracked).toHaveBeenCalledWith("tensorrt_prompt_answered", { accepted: true });
    });
});
