import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import "@/i18n";
import { render } from "@/test/support";
import { IntensityControl } from "./IntensityControl";

/** Draws the control as light adjustment hands it over: -100..100 about 0, carrying 50%. */
const mount = (props: { value?: number; min?: number; max?: number } = { min: -100, max: 100 }) => {
    const onChange = vi.fn();
    render(<IntensityControl label="Bias" value={props.value ?? 50} mark={0} onChange={onChange} {...props} />);

    return onChange;
};

const field = () => screen.getByRole("textbox", { name: "Bias" }) as HTMLInputElement;

const type = (text: string) => fireEvent.change(field(), { target: { value: text } });

/**
 * Drags the thumb across a 200px track from `from` to `to` pixels and lets go, reporting what the field
 * read part-way through.
 *
 * jsdom lays nothing out, so the track is given a rectangle for the one element Radix measures; the
 * pointer capture a drag takes is already stubbed by the shared setup.
 */
const drag = (from: number, through: number, to: number) => {
    const root = document.querySelector("[data-slot='slider']") as HTMLElement;
    vi.spyOn(root, "getBoundingClientRect").mockReturnValue(DOMRect.fromRect({ x: 0, y: 0, width: 200, height: 16 }));

    fireEvent.pointerDown(root, { pointerId: 1, clientX: from });
    fireEvent.pointerMove(root, { pointerId: 1, clientX: through });
    const during = field().value;
    fireEvent.pointerMove(root, { pointerId: 1, clientX: to });
    fireEvent.pointerUp(root, { pointerId: 1, clientX: to });

    return during;
};

describe("the intensity control's typed field", () => {
    it("writes a typed percentage", () => {
        const onChange = mount();

        type("25");

        expect(onChange).toHaveBeenLastCalledWith(25);
    });

    it("holds a bare minus sign until a digit arrives", () => {
        const onChange = mount();

        type("-");

        // A keystroke on the way to a negative number: writing it as 0 would start a run nobody asked for.
        expect(onChange).not.toHaveBeenCalled();
        expect(field().value).toBe("-");

        type("-30");

        expect(onChange).toHaveBeenLastCalledWith(-30);
    });

    it("brings a value beyond the range inside it", () => {
        const onChange = mount();

        type("150");

        expect(onChange).toHaveBeenLastCalledWith(100);
        expect(field().value).toBe("100");
    });

    it("restores what the enhancement carries when an emptied field is left", () => {
        const onChange = mount();

        type("");
        expect(field().value).toBe("");

        fireEvent.blur(field());

        expect(onChange).not.toHaveBeenCalled();
        expect(field().value).toBe("50");
    });

    it("does not clamp before the catalogue has published a range", () => {
        const onChange = mount({ value: 50 });

        type("150");

        expect(onChange).toHaveBeenLastCalledWith(150);
    });
});

describe("the intensity control's slider", () => {
    it("stands at the value the enhancement carries, marked at the neutral value", () => {
        mount({ value: 25, min: -100, max: 100 });

        expect(screen.getByRole("slider", { name: "Bias" })).toHaveAttribute("aria-valuenow", "25");
        expect(screen.getByText("0")).toBeInTheDocument();
    });

    it("shows a drag in the field and writes once, when it is released", () => {
        const onChange = mount();

        // 150px of 200 is 50%, 170px is 70%, 180px is 80% - each on a step of 5.
        const during = drag(150, 170, 180);

        expect(during).toBe("70");
        expect(onChange).toHaveBeenCalledTimes(1);
        expect(onChange).toHaveBeenCalledWith(80);
    });

    it("draws no thumb and takes no input before the catalogue has published a range", () => {
        mount({ value: 50 });

        expect(screen.queryByRole("slider")).not.toBeInTheDocument();
        expect(document.querySelector("[data-slot='slider']")).toHaveAttribute("data-disabled");
    });
});
