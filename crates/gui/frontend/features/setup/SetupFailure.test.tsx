import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { Failure } from "@/ipc/setup";
import { useSetupStore } from "@/stores/setup";
import { alreadyInstalled, apply, finished, PLAN, progress, REASON, render, STATE_SLOTS, states } from "@/test/support";
import { SetupDialog } from "./SetupDialog";

// The one command this dialog calls of its own. Try again is a callback rather than a command,
// because the call site is shared with the mount effect - see `App.tsx`.
vi.mock("@/ipc/setup", () => ({ quit: vi.fn() }));

const { quit } = await import("@/ipc/setup");

/** The failure the store would be holding, put there directly rather than through a rejected promise. */
const failed = (kind: Failure, message = REASON) =>
    useSetupStore.setState({ status: "failed", failure: { kind, message } });

const reasons = () => [...document.querySelectorAll("[data-slot='setup-error-reason']")];

/**
 * The lucide icon each row is drawn with, in order.
 *
 * By the class lucide puts on its own `<svg>` - `lucide lucide-clock` - which is the only handle an
 * icon offers from the DOM, and the one that says *which* icon rather than how it was styled.
 */
const icons = () =>
    [...document.querySelectorAll(STATE_SLOTS)].map(
        (state) =>
            [...(state.closest("div")?.parentElement?.querySelectorAll("svg") ?? [])]
                .flatMap((svg) => [...svg.classList])
                .find((name) => name.startsWith("lucide-")) ?? "none",
    );

/**
 * jsdom ships no `navigator.clipboard`, and it is not something the environment can be asked for: it
 * is a secure-context API the real webview provides. Defined as a property because `navigator` has
 * no setter for it.
 */
const writeText = vi.fn(() => Promise.resolve());

describe("SetupDialog, failed", () => {
    beforeEach(() => {
        useSetupStore.setState({ rows: [], status: "failed", failure: { kind: "transfer", message: REASON } });

        Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
        writeText.mockClear();
    });

    afterEach(() => {
        Reflect.deleteProperty(navigator, "clipboard");
    });

    it("states that setting up could not be finished and what the application needs", () => {
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText("Couldn't finish setting up")).toBeInTheDocument();
        expect(screen.getByText(/Open Photo AI needs these components before it can run/)).toBeInTheDocument();
    });

    it("points at the component that stopped and says what the others did", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            alreadyInstalled("NVIDIA CUDA"),
            progress("NVIDIA cuDNN", "downloading", 0.09),
        );
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // Same components, same order, same length as the setup dialog listed - each stating what it
        // ended up having done rather than what it was doing.
        expect(states()).toEqual(["Installed", "Installed", "Error", "Queued"]);
        for (const row of PLAN.rows) expect(screen.getByText(row.name)).toBeInTheDocument();
        expect(screen.getByText("412 MB")).toBeInTheDocument();
    });

    it("marks the component that stopped, not the first one that finished downloading", () => {
        // The shape a real Windows launch produces: ONNX Runtime was already on disk, CUDA downloaded
        // and expanded, cuDNN stopped. CUDA's last report is `extracting` at 1 - a component that
        // installs never reports `installed` - so reading the state alone would put the Error on CUDA
        // and draw cuDNN, which actually stopped, as a component never reached.
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            progress("NVIDIA cuDNN", "downloading", 0.34),
        );
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Installed", "Error", "Queued"]);
        expect(icons()).toEqual(["lucide-check", "lucide-check", "lucide-triangle-alert", "lucide-clock"]);

        // And the reason sits against that row, which is what makes which failure belongs to which
        // component need no working out.
        expect(reasons()).toHaveLength(1);
        expect(reasons()[0]?.closest("[class*='bg-destructive']")).not.toBeNull();
    });

    it("says and draws about a component what the progress dialog says and draws about it", () => {
        // The invariant, not a duplicate of the assertions above: a component this launch finished
        // with, and one it never reached, are the same facts whichever state the dialog is in - so the
        // row must not rename itself or change its icon as the dialog changes state. Only the one
        // that stopped is the failed state's own.
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.09));

        // The running state first - the store's `failure` is what the dialog switches on, so it has
        // to be absent for this half.
        useSetupStore.setState(({ failure: _cleared, ...state }) => ({ ...state, status: "running" }), true);
        const { unmount } = render(<SetupDialog onRetry={() => {}} />);
        const duringSetup = { states: states(), icons: icons() };
        unmount();

        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // Rows 0 and 2..3 are the same component in the same condition in both dialogs; row 1 is the
        // one that was transferring, which is `Downloading` there and `Error` here.
        expect(duringSetup.states).toEqual(["Installed", "Downloading", "Queued", "Queued"]);
        expect(states()).toEqual(["Installed", "Error", "Queued", "Queued"]);

        expect(duringSetup.icons).toEqual(["lucide-check", "lucide-download", "lucide-clock", "lucide-clock"]);
        expect(icons()).toEqual(["lucide-check", "lucide-triangle-alert", "lucide-clock", "lucide-clock"]);
    });

    it("keeps the overall bar, and it is the only track left", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.41));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // Kept rather than dropped - see `SetupBody`.
        const bars = screen.getAllByRole("progressbar");
        expect(bars).toHaveLength(1);
        expect(screen.getByText("1 of 4 components installed")).toBeInTheDocument();

        // The bar, not a row's: no row has one, because a track measures work under way.
        expect(bars[0]?.closest("[data-slot='setup-row-state']")).toBeNull();
    });

    it("says in the header what became of the attempt, beside the figure the bar carries", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.41));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // Two statements about one attempt, deliberately not the same one, in one run of text carrying
        // both sentences - see the header in `SetupFailure`.
        expect(
            screen.getByText(
                "Open Photo AI needs these components before it can run. 1 of 4 finished; one stopped partway and the rest weren't reached.",
            ),
        ).toBeInTheDocument();

        expect(screen.getByText("1 of 4 components installed")).toBeInTheDocument();
    });

    it("names the last one where nothing was left behind it", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            finished("NVIDIA cuDNN"),
            progress("NVIDIA TensorRT", "downloading", 0.5),
        );
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText(/3 of 4 finished; the last one stopped partway/)).toBeInTheDocument();
    });

    it("states no count where the first component is the one that stopped", () => {
        // "0 of 4" is a figure the bar already carries and says nothing here.
        apply(PLAN, progress("ONNX Runtime", "downloading", 0.12));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText(/The first one stopped partway/)).toBeInTheDocument();
        expect(screen.queryByText(/0 of 4 finished/)).not.toBeInTheDocument();
    });

    it("adds no second sentence where the failure belongs to no component", () => {
        // Every component was already on disk and only the runtime would not start. Nothing stopped
        // part way, so there is nothing to say about a component - and the header keeps the two lines
        // it reserves either way, so the dialog does not change height.
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));
        failed("other");
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText("Open Photo AI needs these components before it can run.")).toBeInTheDocument();
        expect(screen.queryByText(/stopped partway/)).not.toBeInTheDocument();
    });

    it("gives no row a progress track, because no work is under way", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "extracting", 0.85));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Error", "Queued", "Queued"]);

        // One track in the dialog - the overall one - and none in the list, where a track would
        // measure work under way.
        expect(screen.getAllByRole("progressbar")).toHaveLength(1);
        expect(document.querySelectorAll("[data-slot='setup-row-state']")).toHaveLength(4);
    });

    it("carries the whole reason on one line, against the row it belongs to", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.41));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // The whole sentence is in the document even though only one line of it is drawn - two
        // different failures must not read identically, and what is copied is all of it.
        const [reason] = reasons();
        expect(reason).toHaveTextContent(REASON);

        // One line: `truncate` is what ellipsises it, and the 12px box is what makes this row the
        // height the progress dialog's working row is.
        expect(reason).toHaveClass("truncate");
        expect(reason).toHaveClass("h-3");
        expect(reason).toHaveClass("leading-3");

        // The application turns selection off everywhere else; this is the deliberate exception.
        expect(reason).toHaveClass("select-text");

        // Against the row that stopped, which is what makes which failure belongs to which component
        // need no working out.
        expect(reasons()).toHaveLength(1);
        expect(reason?.closest("[class*='bg-destructive']")).not.toBeNull();
    });

    it("copies the whole reason when it is clicked, not the line that was drawn", async () => {
        apply(PLAN, progress("NVIDIA CUDA", "downloading", 0.41));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        const [reason] = reasons();
        expect(reason).toBeDefined();
        if (reason) fireEvent.click(reason);

        expect(writeText).toHaveBeenCalledWith(REASON);
        expect(await screen.findByText("Copied!")).toBeInTheDocument();
    });

    it("takes Copied! back down after a moment", async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });

        try {
            apply(PLAN, progress("NVIDIA CUDA", "downloading", 0.41));
            failed("transfer");
            render(<SetupDialog onRetry={() => {}} />);

            const [reason] = reasons();
            if (reason) fireEvent.click(reason);
            expect(await screen.findByText("Copied!")).toBeInTheDocument();

            await act(async () => {
                vi.advanceTimersByTime(2_000);
            });

            await waitFor(() => expect(screen.queryByText("Copied!")).not.toBeInTheDocument());
        } finally {
            vi.useRealTimers();
        }
    });

    it("says nothing when there is no clipboard to write to", () => {
        // Nothing produces this in the webview, which is a secure context - but a click handler that
        // threw would take the dialog down with it, which is the one thing a failure dialog must not
        // do.
        Reflect.deleteProperty(navigator, "clipboard");

        apply(PLAN, progress("NVIDIA CUDA", "downloading", 0.41));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        const [reason] = reasons();
        if (reason) fireEvent.click(reason);

        expect(screen.queryByText("Copied!")).not.toBeInTheDocument();
        expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    });

    it("marks no row and shows the reason on its own when the failure belongs to no component", () => {
        // Every component already on disk and only the runtime would not start, which is the common
        // case on a machine that has run this application before.
        apply(PLAN, ...PLAN.rows.map((row) => alreadyInstalled(row.name)));
        failed("other", "failed to start the ONNX Runtime environment");
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Installed", "Installed", "Installed"]);
        expect(reasons()).toHaveLength(1);
        expect(screen.getByText("failed to start the ONNX Runtime environment")).toBeInTheDocument();
    });

    it("draws a failure raised before anything was planned as the reason alone", () => {
        failed("other", "Open Photo AI is already running (gui, pid 4821); close it and try again");
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual([]);
        expect(reasons()).toHaveLength(1);
    });

    it.each([
        ["transfer", true, true],
        ["unrecoverable", false, false],
        ["other", false, true],
    ] as const)("names the known cause and offers a retry as %s requires", (kind, note, retry) => {
        apply(PLAN, progress("ONNX Runtime", "downloading", 0.2));
        failed(kind);
        render(<SetupDialog onRetry={() => {}} />);

        // The throttling advice only where getting the files was the problem, and Try again only
        // where a second attempt could succeed. Both read the one value rather than a rule of their
        // own.
        expect(screen.queryByText(/hosted on GitHub, which throttles downloads/) !== null).toBe(note);
        expect(screen.queryByRole("button", { name: "Try again" }) !== null).toBe(retry);

        // Quit is there whatever the failure was, and is the only action when Try again is withheld.
        expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
    });

    it("cannot be dismissed by Escape, by clicking away, or by a close button", () => {
        apply(PLAN, progress("ONNX Runtime", "downloading", 0.2));
        failed("transfer");
        render(<SetupDialog onRetry={() => {}} />);

        // All three of Radix's ways out are turned off explicitly on `SetupDialog`'s one
        // `DialogContent` - the close button by `showCloseButton`, Escape and the outside click by
        // their handlers - so this asserts the same three in the failed state as the progress state.
        // The role is `alertdialog` here and `dialog` there, which is the state's own property.
        const dialog = screen.getByRole("alertdialog");
        expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();

        fireEvent.keyDown(document, { key: "Escape" });
        expect(dialog).toBeInTheDocument();

        fireEvent.pointerDown(document.body);
        fireEvent.click(document.body);
        expect(screen.getByRole("alertdialog")).toBeInTheDocument();

        // And it survived with what it had, rather than as an empty dialog.
        expect(states()).toEqual(["Error", "Queued", "Queued", "Queued"]);
    });

    it("asks Rust to end the application when Quit is chosen", () => {
        render(<SetupDialog onRetry={() => {}} />);

        fireEvent.click(screen.getByRole("button", { name: "Quit" }));

        expect(quit).toHaveBeenCalledTimes(1);
    });

    it("hands Try again back to its caller rather than starting an initialization of its own", () => {
        const onRetry = vi.fn();
        render(<SetupDialog onRetry={onRetry} />);

        fireEvent.click(screen.getByRole("button", { name: "Try again" }));

        expect(onRetry).toHaveBeenCalledTimes(1);
    });
});
