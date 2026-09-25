import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "@/i18n";
import { MIN_CROP_SIZE } from "@/lib/constants";
import { render } from "@/test/support";
import { CropDimensions } from "./CropDimensions";

const handlers = () => ({ onWidthCommit: vi.fn(), onHeightCommit: vi.fn(), onSwap: vi.fn() });

describe("CropDimensions", () => {
    it("shows the rectangle in the photograph's own pixels", () => {
        render(<CropDimensions width={2560} height={1440} {...handlers()} />);

        expect(screen.getByRole("textbox", { name: "w" })).toHaveValue("2560");
        expect(screen.getByRole("textbox", { name: "h" })).toHaveValue("1440");
    });

    it("commits each keystroke so the rectangle tracks what is being typed", () => {
        const spies = handlers();
        render(<CropDimensions width={2560} height={1440} {...spies} />);

        fireEvent.change(screen.getByRole("textbox", { name: "w" }), { target: { value: "1600" } });

        expect(spies.onWidthCommit).toHaveBeenCalledWith(1600);
    });

    it("is not clobbered mid-typing by an incoming framing", () => {
        const spies = handlers();
        const view = render(<CropDimensions width={2560} height={1440} {...spies} />);
        const field = screen.getByRole("textbox", { name: "w" });

        fireEvent.focus(field);
        fireEvent.change(field, { target: { value: "25" } });

        // The stencil answering back mid-keystroke, which is exactly what happens in the real dialog.
        view.rerender(<CropDimensions width={25} height={1440} {...spies} />);

        expect(field).toHaveValue("25");
    });

    it("snaps back to what the framing actually is once it is left", () => {
        const spies = handlers();
        const view = render(<CropDimensions width={2560} height={1440} {...spies} />);
        const field = screen.getByRole("textbox", { name: "w" });

        fireEvent.focus(field);
        fireEvent.change(field, { target: { value: "2560" } });
        fireEvent.blur(field);

        // The one-pixel snap a reduced rendition costs, contained to the blur: the widget reports 2561
        // back for what was typed as 2560 on a photograph the bound reduced.
        view.rerender(<CropDimensions width={2561} height={1440} {...spies} />);

        expect(field).toHaveValue("2561");
    });

    it("falls back to the minimum when an emptied field is left", () => {
        const spies = handlers();
        render(<CropDimensions width={2560} height={1440} {...spies} />);
        const field = screen.getByRole("textbox", { name: "w" });

        fireEvent.focus(field);
        fireEvent.change(field, { target: { value: "" } });
        fireEvent.blur(field);

        expect(spies.onWidthCommit).toHaveBeenLastCalledWith(MIN_CROP_SIZE);
    });

    it("does not fight a field being emptied on the way to another value", () => {
        const spies = handlers();
        render(<CropDimensions width={2560} height={1440} {...spies} />);
        const field = screen.getByRole("textbox", { name: "w" });

        fireEvent.focus(field);
        fireEvent.change(field, { target: { value: "" } });

        expect(spies.onWidthCommit).not.toHaveBeenCalled();
    });

    it("carries a named swap between the two, which the reference leaves unnamed", () => {
        const spies = handlers();
        render(<CropDimensions width={2560} height={1440} {...spies} />);

        fireEvent.click(screen.getByRole("button", { name: "Swap width and height" }));

        expect(spies.onSwap).toHaveBeenCalledTimes(1);
    });
});
