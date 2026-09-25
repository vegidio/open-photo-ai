import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "@/i18n";
import { render } from "@/test/support";
import { RotateControls } from "./RotateControls";

const handlers = () => ({
    onRotationChange: vi.fn(),
    onRotate90: vi.fn(),
    onFlipHorizontal: vi.fn(),
    onFlipVertical: vi.fn(),
    onReset: vi.fn(),
});

describe("RotateControls", () => {
    it("runs the fine rotation from a quarter turn either way", () => {
        render(<RotateControls rotation={0} {...handlers()} />);

        const slider = screen.getByRole("slider");

        expect(slider).toHaveAttribute("aria-valuemin", "-90");
        expect(slider).toHaveAttribute("aria-valuemax", "90");
    });

    it("prints the zero of the scale and nothing else", () => {
        render(<RotateControls rotation={0} {...handlers()} />);

        // A divergence from the design, which prints all three - `RotateControls.tsx` says why.
        expect(screen.getByText("0°")).toBeInTheDocument();
        expect(screen.queryByText("-90°")).not.toBeInTheDocument();
        expect(screen.queryByText("90°")).not.toBeInTheDocument();
    });

    it("hangs that label out of flow, so the slider stays on the row's centreline", () => {
        render(<RotateControls rotation={0} {...handlers()} />);

        expect(screen.getByText("0°").className).toContain("absolute");
    });

    it("marks the scale every fifteen degrees", () => {
        const { container } = render(<RotateControls rotation={0} {...handlers()} />);

        // Thirteen: the two ends and the eleven between them, which is what 15 degrees across 180
        // comes to. A mark every 15 is the design's, and is what makes the slider readable as
        // degrees rather than as a fraction of a bar.
        expect(container.querySelectorAll("[aria-hidden='true'] span")).toHaveLength(13);
    });

    it("draws no filled track, since it turns about a meaningful zero", () => {
        const { container } = render(<RotateControls rotation={0} {...handlers()} />);

        expect(container.querySelector("[data-slot='slider-range']")).toBeNull();
    });

    it("says where the rotation currently is", () => {
        render(<RotateControls rotation={12} {...handlers()} />);

        expect(screen.getByRole("slider")).toHaveAttribute("aria-valuenow", "12");
    });

    it("reaches each handler from its own control", () => {
        const spies = handlers();
        render(<RotateControls rotation={0} {...spies} />);

        for (const [name, spy] of [
            ["Rotate 90 degrees", spies.onRotate90],
            ["Flip horizontally", spies.onFlipHorizontal],
            ["Flip vertically", spies.onFlipVertical],
            ["Reset", spies.onReset],
        ] as const) {
            fireEvent.click(screen.getByRole("button", { name }));
            expect(spy, `${name} did not reach its handler`).toHaveBeenCalledTimes(1);
        }
    });

    it("names every icon-only control, which the reference does not", () => {
        render(<RotateControls rotation={0} {...handlers()} />);

        for (const name of ["Rotate 90 degrees", "Flip horizontally", "Flip vertically"]) {
            expect(screen.getByRole("button", { name })).toBeInTheDocument();
        }
    });
});
