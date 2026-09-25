import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { useSetupStore } from "@/stores/setup";
import { alreadyInstalled, apply, finished, PLAN, progress, render, SINGLE_PLAN, states } from "@/test/support";
import { SetupDialog } from "./SetupDialog";

// The one command this dialog calls. Mocked because the real one is an `invoke`, and because the
// assertion worth making is that Quit asks Rust to exit rather than doing anything of its own.
vi.mock("@/ipc/setup", () => ({ quit: vi.fn() }));

const { quit } = await import("@/ipc/setup");

describe("SetupDialog, running", () => {
    beforeEach(() => {
        useSetupStore.setState({ rows: [], status: "running" });
    });

    it("draws every state the design lists, one row each", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            progress("NVIDIA CUDA", "downloading", 0.41),
            progress("NVIDIA cuDNN", "extracting", 0.85),
        );
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Downloading", "Extracting", "Queued"]);

        // Each row's name, unchanged and untranslated, beside its published size.
        for (const row of PLAN.rows) expect(screen.getByText(row.name)).toBeInTheDocument();
        expect(screen.getByText("184 MB")).toBeInTheDocument();
        expect(screen.getByText("612 MB")).toBeInTheDocument();
        expect(screen.getByText("1 MB")).toBeInTheDocument();
    });

    it("gives a track only to the components with work in flight", () => {
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            progress("NVIDIA CUDA", "downloading", 0.41),
            progress("NVIDIA cuDNN", "extracting", 0.85),
        );
        render(<SetupDialog onRetry={() => {}} />);

        // Three bars: the overall one plus one each for the downloading and extracting rows. The
        // installed and queued rows have none - a track at zero would say a transfer had started.
        expect(screen.getAllByRole("progressbar")).toHaveLength(3);
    });

    it("hands one component over to the next in a single moment", () => {
        // The gap first: CUDA has landed on 1 in the phase it finished in, and cuDNN has reported
        // nothing yet. Its row keeps saying `Extracting` for as long as the library is between
        // components, rather than going quiet and leaving a list where nothing is happening.
        apply(PLAN, alreadyInstalled("ONNX Runtime"), finished("NVIDIA CUDA"));
        const { unmount } = render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Extracting", "Queued", "Queued"]);
        unmount();

        // Then the report that starts cuDNN, which is the one report that changes both rows: never a
        // frame with two components working, and never one with none.
        apply(progress("NVIDIA cuDNN", "downloading", 0.06));
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Installed", "Downloading", "Queued"]);

        // And the finished row gives its track up with its label: two bars, the overall one and
        // cuDNN's. A full track on a finished row is the other half of what said it was still working.
        expect(screen.getAllByRole("progressbar")).toHaveLength(2);
    });

    it("installs the last component as soon as it finishes", () => {
        // Nothing follows it to hand over to, and the dialog stays up while the runtime is loaded.
        apply(
            PLAN,
            alreadyInstalled("ONNX Runtime"),
            finished("NVIDIA CUDA"),
            finished("NVIDIA cuDNN"),
            finished("NVIDIA TensorRT"),
        );
        render(<SetupDialog onRetry={() => {}} />);

        expect(states()).toEqual(["Installed", "Installed", "Installed", "Installed"]);
        expect(screen.getAllByRole("progressbar")).toHaveLength(1);
    });

    it("says how many components are installed out of every component the machine needs", () => {
        apply(PLAN, alreadyInstalled("ONNX Runtime"), progress("NVIDIA CUDA", "downloading", 0.41));
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText("1 of 4 components installed")).toBeInTheDocument();

        // Weighted by size: 184 MB done plus 41% of 612 MB, over 1209 MB, is 36% - not the 25% a
        // count of finished components would show.
        expect(screen.getByText("36%")).toBeInTheDocument();
    });

    it("pluralises the count against the number of components, not the number installed", () => {
        apply(SINGLE_PLAN);
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText("0 of 1 component installed")).toBeInTheDocument();
    });

    it("explains why setting up can be slow", () => {
        apply(PLAN);
        render(<SetupDialog onRetry={() => {}} />);

        expect(screen.getByText(/hosted on GitHub, which throttles downloads/)).toBeInTheDocument();
    });

    it("cannot be dismissed by Escape, by clicking away, or by a close button", () => {
        apply(PLAN, progress("NVIDIA CUDA", "downloading", 0.41));
        render(<SetupDialog onRetry={() => {}} />);

        const dialog = screen.getByRole("dialog");
        expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();

        fireEvent.keyDown(document, { key: "Escape" });
        expect(dialog).toBeInTheDocument();

        // Radix closes on `pointerdown` outside, which is what the prevented handler intercepts.
        fireEvent.pointerDown(document.body);
        fireEvent.click(document.body);
        expect(screen.getByRole("dialog")).toBeInTheDocument();

        // And the rows are still what they were, rather than a dialog that survived empty.
        expect(states()).toEqual(["Queued", "Downloading", "Queued", "Queued"]);
    });

    it("asks Rust to end the application when Quit is chosen", () => {
        apply(PLAN);
        render(<SetupDialog onRetry={() => {}} />);

        fireEvent.click(screen.getByRole("button", { name: "Quit" }));

        expect(quit).toHaveBeenCalledTimes(1);
    });
});
